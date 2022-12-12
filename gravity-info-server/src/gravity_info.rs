//! This thread handles querying Gravity Bridge and Ethereum for information
//! and using this info to update global lazy static data in memory, this decouples requests
//! for info from the actual info gathering and makes queries dramatically more scalable.

use actix_web::rt::System;
use clarity::{Address as EthAddress, Uint256};
use cosmos_gravity::query::{
    get_attestations, get_gravity_params, get_latest_transaction_batches, get_pending_batch_fees,
};
use deep_space::{Address, Coin, Contact};
use futures::future::{join, join5, join_all};
use futures::join;
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_proto::gravity::{
    Attestation, BatchFees, Params as GravityParams, QueryDenomToErc20Request,
};
use gravity_utils::error::GravityError;
use gravity_utils::types::{event_signatures::*, *};
use gravity_utils::types::{SendToCosmosEvent, TransactionBatch};
use log::{error, info, trace};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use tonic::transport::channel::Channel;
use web30::amm::USDC_CONTRACT_ADDRESS;
use web30::client::Web3;

const LOOP_TIME: Duration = Duration::from_secs(30);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const GRAVITY_NODE_GRPC: &str = "http://gravitychain.io:9090";
pub const GRAVITY_PREFIX: &str = "gravity";
pub const ETH_NODE_RPC: &str = "https://eth.althea.net";
pub const FINALITY_DELAY: u128 = 100;
/// number of seconds per eth block
pub const ETH_BLOCK_TIME: u128 = 12;

/// In memory store of gravity state used to serve rpc requests
#[derive(Debug, Default, Clone, Serialize)]
pub struct GravityInfo {
    /// Pending transactions from Gravity Bridge to Ethereum
    pub pending_tx: Vec<InternalBatchFees>,
    pub pending_batches: Vec<TransactionBatch>,
    pub attestations: Vec<InteralAttestation>,
    pub params: InternalGravityParams,
}

/// In memory store of Ethereum state used to serve rpc requests
#[derive(Debug, Default, Clone, Serialize)]
pub struct EthInfo {
    pub deposit_events: Vec<DepositWithMetadata>,
    pub batch_events: Vec<TransactionBatchExecutedEvent>,
    pub valset_updates: Vec<ValsetUpdatedEvent>,
    pub erc20_deploys: Vec<Erc20DeployedEvent>,
    pub logic_calls: Vec<LogicCallExecutedEvent>,
    pub latest_eth_block: Uint256,
}

lazy_static! {
    static ref GRAVITY_INFO: Arc<RwLock<Option<GravityInfo>>> = Arc::new(RwLock::new(None));
    static ref ETH_INFO: Arc<RwLock<Option<EthInfo>>> = Arc::new(RwLock::new(None));
    static ref ERC20_METADATA: Arc<RwLock<Option<Vec<Erc20Metadata>>>> =
        Arc::new(RwLock::new(None));
}

pub fn get_gravity_info() -> Option<GravityInfo> {
    GRAVITY_INFO.read().unwrap().clone()
}

fn set_gravity_info(info: GravityInfo) {
    let mut lock = GRAVITY_INFO.write().unwrap();
    *lock = Some(info)
}

pub fn get_eth_info() -> Option<EthInfo> {
    ETH_INFO.read().unwrap().clone()
}

fn set_eth_info(info: EthInfo) {
    let mut lock = ETH_INFO.write().unwrap();
    *lock = Some(info)
}

pub fn get_erc20_metadata() -> Option<Vec<Erc20Metadata>> {
    ERC20_METADATA.read().unwrap().clone()
}

fn set_erc20_metadata(metadata: Vec<Erc20Metadata>) {
    let mut lock = ERC20_METADATA.write().unwrap();
    *lock = Some(metadata)
}

pub fn blockchain_info_thread() {
    info!("Starting Gravity info watcher");

    thread::spawn(move || loop {
        let runner = System::new();
        runner.block_on(async move {
            let web30 = Web3::new(ETH_NODE_RPC, REQUEST_TIMEOUT);
            let contact = Contact::new(GRAVITY_NODE_GRPC, REQUEST_TIMEOUT, GRAVITY_PREFIX).unwrap();
            // since we're rebuilding the async env every loop iteration we need to re-init this
            let mut grpc_client = GravityQueryClient::connect(GRAVITY_NODE_GRPC)
                .await
                .unwrap();

            let gravity_contract_address =
                match query_gravity_info(&contact, &mut grpc_client).await {
                    Ok(v) => {
                        let bridge_eth_address = v.params.bridge_ethereum_address;
                        set_gravity_info(v);
                        bridge_eth_address
                    }
                    Err(e) => {
                        error!("Failed to update Gravity Info with {:?}", e);
                        return;
                    }
                };
            let eth_info = query_eth_info(&web30, gravity_contract_address);
            let erc20_metadata = get_all_erc20_metadata(&contact, &web30, &mut grpc_client);
            let (eth_info, erc20_metadata) = join!(eth_info, erc20_metadata);
            let (eth_info, erc20_metadata) = match (eth_info, erc20_metadata) {
                (Ok(a), Ok(b)) => (a, b),
                (_, Err(e)) => {
                    error!("Failed to get eth info {:?}", e);
                    return;
                }
                (Err(e), _) => {
                    error!("Failed to get erc20 metadata {:?}", e);
                    return;
                }
            };

            set_eth_info(eth_info);
            set_erc20_metadata(erc20_metadata);
            info!("Successfully updated Gravity and ETH info");
        });
        thread::sleep(LOOP_TIME);
    });
}

/// gets information about all tokens that have been bridged
async fn get_all_erc20_metadata(
    contact: &Contact,
    web30: &Web3,
    grpc_client: &mut GravityQueryClient<Channel>,
) -> Result<Vec<Erc20Metadata>, GravityError> {
    let all_tokens_on_gravity = contact.query_total_supply().await?;
    let mut futs = Vec::new();
    for token in all_tokens_on_gravity {
        let erc20: EthAddress = if token.denom.starts_with("gravity") {
            token.denom.trim_start_matches("gravity").parse().unwrap()
        } else {
            match grpc_client
                .denom_to_erc20(QueryDenomToErc20Request { denom: token.denom })
                .await
            {
                Ok(v) => v.into_inner().erc20.parse().unwrap(),
                Err(_) => continue,
            }
        };
        futs.push(get_metadata(web30, erc20));
    }
    let results = join_all(futs).await;
    let mut metadata = Vec::new();
    for r in results {
        metadata.push(r?)
    }

    Ok(metadata)
}

async fn get_metadata(web30: &Web3, erc20: EthAddress) -> Result<Erc20Metadata, GravityError> {
    let query_sender: EthAddress = "0x388C818CA8B9251b393131C08a736A67ccB19297"
        .parse()
        .unwrap();
    let symbol = web30.get_erc20_symbol(erc20, query_sender);
    let decimals = web30.get_erc20_decimals(erc20, query_sender);
    let (symbol, decimals) = join(symbol, decimals).await;
    let (symbol, decimals) = (symbol?, decimals?);

    // the token is USDC, no more querying required
    if erc20 == *USDC_CONTRACT_ADDRESS {
        return Ok(Erc20Metadata {
            address: erc20,
            symbol,
            decimals,
            exchange_rate: Some(10u128.pow(6).into()),
        });
    }

    let downcast_decimals: u32 = decimals.to_string().parse().unwrap();
    // one of whatever this token is
    let one: Uint256 = 10u128.pow(downcast_decimals).into();

    let pricev3 = web30.get_uniswap_v3_price_with_retries(
        query_sender,
        erc20,
        *USDC_CONTRACT_ADDRESS,
        one.clone(),
        None,
        None,
    );
    let pricev2 =
        web30.get_uniswap_v2_price(query_sender, erc20, *USDC_CONTRACT_ADDRESS, one, None);

    let (pricev3, pricev2) = join(pricev3, pricev2).await;

    // the value of one unit of whatever this is in usdc
    let exchange_rate = match (pricev3, pricev2) {
        (Ok(r), _) => Some(r),
        (_, Ok(r)) => Some(r),
        (Err(_), Err(_)) => None,
    };
    Ok(Erc20Metadata {
        address: erc20,
        symbol,
        decimals,
        exchange_rate,
    })
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Erc20Metadata {
    pub address: EthAddress,
    pub decimals: Uint256,
    pub symbol: String,
    /// the amount of this token worth one DAI (one dollar)
    pub exchange_rate: Option<Uint256>,
}

async fn query_gravity_info(
    _contact: &Contact,
    grpc_client: &mut GravityQueryClient<Channel>,
) -> Result<GravityInfo, GravityError> {
    // can't be easily parallelized becuase of the grpc client :(
    let pending_tx = get_pending_batch_fees(grpc_client).await?.batch_fees;
    let pending_batches = get_latest_transaction_batches(grpc_client).await?;
    let attestations = get_attestations(grpc_client, None).await?;
    let params = get_gravity_params(grpc_client).await?;

    Ok(GravityInfo {
        pending_tx: pending_tx.into_iter().map(|b| b.into()).collect(),
        pending_batches,
        attestations: attestations.into_iter().map(|a| a.into()).collect(),
        params: params.into(),
    })
}

/// A serializable version of the batch fees struct
#[derive(Debug, Default, Clone, Serialize)]
pub struct InternalBatchFees {
    pub token: EthAddress,
    pub total_fees: Uint256,
    pub tx_count: u64,
}

impl From<BatchFees> for InternalBatchFees {
    fn from(b: BatchFees) -> Self {
        InternalBatchFees {
            token: b.token.parse().unwrap(),
            total_fees: b.total_fees.parse().unwrap(),
            tx_count: b.tx_count,
        }
    }
}

/// A seriializable version of the Attestation struct
#[derive(Debug, Default, Clone, Serialize)]
pub struct InteralAttestation {
    pub height: u64,
    pub observed: bool,
    pub votes: u64,
}

impl From<Attestation> for InteralAttestation {
    fn from(a: Attestation) -> Self {
        InteralAttestation {
            height: a.height,
            observed: a.observed,
            votes: a.votes.len() as u64,
        }
    }
}

/// A drop in for SendToCosmosEvent that provies more useful metadata to the user
#[derive(Serialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct DepositWithMetadata {
    pub erc20: EthAddress,
    pub sender: EthAddress,
    pub destination: Address,
    pub amount: Uint256,
    pub event_nonce: u64,
    pub block_height: Uint256,
    // if this deposit is not yet executed on GB this will be false
    pub confirmed: bool,
    pub blocks_until_confirmed: Uint256,
    pub seconds_until_confirmed: Uint256,
}

impl DepositWithMetadata {
    pub fn convert(input: SendToCosmosEvent, current_eth_height: Uint256) -> Option<Self> {
        let finished =
            current_eth_height.clone() - input.block_height.clone() > FINALITY_DELAY.into();
        // height at which Gravity will see this tx
        let confirm_height = input.block_height.clone() + FINALITY_DELAY.into();
        let blocks_until_confirmed: Uint256 = if finished {
            0u8.into()
        } else {
            confirm_height - current_eth_height
        };

        if let Some(destination) = input.validated_destination {
            Some(DepositWithMetadata {
                erc20: input.erc20,
                sender: input.sender,
                destination,
                amount: input.amount,
                event_nonce: input.event_nonce,
                block_height: input.block_height,
                confirmed: finished,
                blocks_until_confirmed: blocks_until_confirmed.clone(),
                seconds_until_confirmed: blocks_until_confirmed * ETH_BLOCK_TIME.into(),
            })
        } else {
            None
        }
    }
}

/// A serializable version of the Gravity Params
#[derive(Debug, Default, Clone, Serialize)]
pub struct InternalGravityParams {
    pub bridge_ethereum_address: EthAddress,
    pub average_block_time: u64,
    pub average_ethereum_block_time: u64,
    pub target_batch_timeout: u64,
    pub bridge_active: bool,
    pub ethereum_blacklist: Vec<EthAddress>,
    pub gravity_id: String,
    pub bridge_chain_id: u64,
    pub signed_valsets_window: u64,
    pub signed_batches_window: u64,
    pub signed_logic_calls_window: u64,
    pub unbond_slashing_valsets_window: u64,
    pub valset_reward: Option<Coin>,
}

impl From<GravityParams> for InternalGravityParams {
    fn from(p: GravityParams) -> Self {
        InternalGravityParams {
            bridge_ethereum_address: p.bridge_ethereum_address.parse().unwrap(),
            average_block_time: p.average_block_time,
            average_ethereum_block_time: p.average_ethereum_block_time,
            bridge_active: p.bridge_active,
            target_batch_timeout: p.target_batch_timeout,
            ethereum_blacklist: p
                .ethereum_blacklist
                .into_iter()
                .map(|a| a.parse().unwrap())
                .collect(),
            gravity_id: p.gravity_id,
            bridge_chain_id: p.bridge_chain_id,
            signed_valsets_window: p.signed_valsets_window,
            signed_batches_window: p.signed_batches_window,
            signed_logic_calls_window: p.signed_logic_calls_window,
            unbond_slashing_valsets_window: p.unbond_slashing_valsets_window,
            valset_reward: p.valset_reward.map(|c| c.into()),
        }
    }
}

async fn query_eth_info(
    web3: &Web3,
    gravity_contract_address: EthAddress,
) -> Result<EthInfo, GravityError> {
    let latest_block = web3.eth_block_number().await?;
    let starting_block = latest_block.clone() - 7_200u16.into();

    let deposits = web3.check_for_events(
        starting_block.clone(),
        Some(latest_block.clone()),
        vec![gravity_contract_address],
        vec![SENT_TO_COSMOS_EVENT_SIG],
    );
    let batches = web3.check_for_events(
        starting_block.clone(),
        Some(latest_block.clone()),
        vec![gravity_contract_address],
        vec![TRANSACTION_BATCH_EXECUTED_EVENT_SIG],
    );
    let valsets = web3.check_for_events(
        starting_block.clone(),
        Some(latest_block.clone()),
        vec![gravity_contract_address],
        vec![VALSET_UPDATED_EVENT_SIG],
    );
    let erc20_deployed = web3.check_for_events(
        starting_block.clone(),
        Some(latest_block.clone()),
        vec![gravity_contract_address],
        vec![ERC20_DEPLOYED_EVENT_SIG],
    );
    let logic_call_executed = web3.check_for_events(
        starting_block.clone(),
        Some(latest_block.clone()),
        vec![gravity_contract_address],
        vec![LOGIC_CALL_EVENT_SIG],
    );
    let (deposits, batches, valsets, erc20_deployed, logic_call_executed) = join5(
        deposits,
        batches,
        valsets,
        erc20_deployed,
        logic_call_executed,
    )
    .await;

    let (deposits, batches, valsets, erc20_deployed, logic_call_executed) = (
        deposits?,
        batches?,
        valsets?,
        erc20_deployed?,
        logic_call_executed?,
    );

    let valsets = ValsetUpdatedEvent::from_logs(&valsets)?;
    trace!("parsed valsets {:?}", valsets);
    let withdraws = TransactionBatchExecutedEvent::from_logs(&batches)?;
    trace!("parsed batches {:?}", batches);
    let deposits = SendToCosmosEvent::from_logs(&deposits)?;
    trace!("parsed deposits {:?}", deposits);
    let erc20_deploys = Erc20DeployedEvent::from_logs(&erc20_deployed)?;
    trace!("parsed erc20 deploys {:?}", erc20_deploys);
    let logic_calls = LogicCallExecutedEvent::from_logs(&logic_call_executed)?;
    trace!("logic call executions {:?}", logic_calls);

    let mut deposit_events = Vec::new();
    for d in deposits {
        let d = DepositWithMetadata::convert(d, latest_block.clone());
        if let Some(d) = d {
            deposit_events.push(d);
        }
    }

    Ok(EthInfo {
        deposit_events,
        batch_events: withdraws,
        valset_updates: valsets,
        erc20_deploys,
        logic_calls,
        latest_eth_block: latest_block,
    })
}
