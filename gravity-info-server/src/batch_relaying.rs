use actix_web::{HttpResponse, Responder};
use clarity::{Address as EthAddress, Uint256};
use cosmos_gravity::query::{
    get_gravity_params, get_latest_transaction_batches, get_transaction_batch_signatures,
};
use deep_space::Contact;
use ethereum_gravity::message_signatures::encode_tx_batch_confirm_hashed;
use ethereum_gravity::utils::encode_valset_struct;
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::error::GravityError;
use gravity_utils::types::{to_arrays, BatchConfirmResponse, TransactionBatch, Valset};
use log::{error, trace};
use relayer::find_latest_valset::find_latest_valset;
use std::time::Duration;
use web30::client::Web3;

use crate::gravity_info::{ETH_NODE_RPC, GRAVITY_NODE_GRPC, GRAVITY_PREFIX, REQUEST_TIMEOUT};

pub async fn generate_raw_batch_tx(batch_nonce: u64) -> impl Responder {
    let web3 = Web3::new(ETH_NODE_RPC, REQUEST_TIMEOUT);
    let _contact = Contact::new(GRAVITY_NODE_GRPC, REQUEST_TIMEOUT, GRAVITY_PREFIX).unwrap();
    let mut grpc = loop {
        match GravityQueryClient::connect(GRAVITY_NODE_GRPC).await {
            Ok(client) => break client,
            Err(e) => {
                error!("Failed to connect to the GRPC server: {:?}", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    };
    let params = match get_gravity_params(&mut grpc).await {
        Ok(p) => p,
        Err(_) => return HttpResponse::InternalServerError().json("Failed to get gravity params!"),
    };
    let gravity_bridge_address: EthAddress = match params.bridge_ethereum_address.parse() {
        Ok(a) => a,
        Err(_) => return HttpResponse::InternalServerError().json("Interanl error"),
    };

    // find the target batch and check that it's not timed out
    let latest_eth_height = match web3.eth_block_number().await {
        Ok(bn) => bn,
        Err(_) => {
            return HttpResponse::InternalServerError().json("Failed to get latest eth height")
        }
    };
    let latest_batches = match get_latest_transaction_batches(&mut grpc).await {
        Ok(v) => v,
        Err(_) => return HttpResponse::InternalServerError().json("Failed to get batches!"),
    };
    let mut target_batch: Option<TransactionBatch> = None;
    for current_batch in latest_batches {
        if current_batch.nonce == batch_nonce {
            if Uint256::from(current_batch.batch_timeout) < latest_eth_height {
                return HttpResponse::BadRequest().json("Batch has timed out!");
            }
            target_batch = Some(current_batch);
            break;
        }
    }
    let target_batch = match target_batch {
        Some(b) => b,

        None => return HttpResponse::BadRequest().json("Batch nonce not found!"),
    };

    let sigs = get_transaction_batch_signatures(
        &mut grpc,
        target_batch.nonce,
        target_batch.token_contract,
    )
    .await
    .expect("Failed to get sigs for batch!");
    if sigs.is_empty() {
        return HttpResponse::InternalServerError().json("Failed to get sigs for batch");
    }

    let current_valset = find_latest_valset(&mut grpc, gravity_bridge_address, &web3).await;
    if current_valset.is_err() {
        error!("Could not get current valset! {:?}", current_valset);
        return HttpResponse::InternalServerError().json("Failed tog et sigs for batch");
    }
    let current_valset = current_valset.unwrap();

    // this checks that the signatures for the batch are actually possible to submit to the chain
    let hash = encode_tx_batch_confirm_hashed(params.gravity_id.clone(), target_batch.clone());

    if let Err(e) = current_valset.order_sigs(&hash, &sigs) {
        error!("Current validator set is not valid to relay this batch, a validator set update must be submitted!");
        error!("{:?}", e);
        return HttpResponse::InternalServerError().json("sig order not valid");
    }

    match encode_batch_payload(current_valset, &target_batch, &sigs, params.gravity_id) {
        Ok(payload) => HttpResponse::Ok().json(payload),
        Err(_) => HttpResponse::InternalServerError().json("Failed to encode payload!"),
    }
}

// TODO make this function in ethereum_gravity public
fn encode_batch_payload(
    current_valset: Valset,
    batch: &TransactionBatch,
    confirms: &[BatchConfirmResponse],
    gravity_id: String,
) -> Result<Vec<u8>, GravityError> {
    let current_valset_token = encode_valset_struct(&current_valset);
    let new_batch_nonce = batch.nonce;
    let hash = encode_tx_batch_confirm_hashed(gravity_id, batch.clone());
    let sig_data = current_valset.order_sigs(&hash, confirms)?;
    let sig_arrays = to_arrays(sig_data);
    let (amounts, destinations, fees) = batch.get_checkpoint_values();

    // Solidity function signature
    // function submitBatch(
    // // The validators that approve the batch and new valset encoded as a ValsetArgs struct
    // address[] memory _currentValidators,
    // uint256[] memory _currentPowers,
    // uint256 _currentValsetNonce,
    // uint256 _rewardAmount,
    // address _rewardToken,
    //
    // // These are arrays of the parts of the validators signatures
    // uint8[] memory _v,
    // bytes32[] memory _r,
    // bytes32[] memory _s,
    // // The batch of transactions
    // uint256[] memory _amounts,
    // address[] memory _destinations,
    // uint256[] memory _fees,
    // uint256 _batchNonce,
    // address _tokenContract,
    // uint256 _batchTimeout
    let tokens = &[
        current_valset_token,
        sig_arrays.sigs,
        amounts,
        destinations,
        fees,
        new_batch_nonce.into(),
        batch.token_contract.into(),
        batch.batch_timeout.into(),
    ];
    let payload = clarity::abi::encode_call("submitBatch((address[],uint256[],uint256,uint256,address),(uint8,bytes32,bytes32)[],uint256[],address[],uint256[],uint256,address,uint256)",
    tokens).unwrap();
    trace!("Tokens {:?}", tokens);

    Ok(payload)
}
