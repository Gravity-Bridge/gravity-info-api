//! This thread handles querying Gravity Bridge and Ethereum for information
//! and using this info to update global lazy static data in memory, this decouples requests
//! for info from the actual info gathering and makes queries dramatically more scalable.

use actix_web::rt::System;
use cosmos_gravity::query::{
    get_attestations, get_latest_transaction_batches, get_pending_batch_fees,
};
use deep_space::{Contact};
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_proto::gravity::{Attestation, BatchFees};
use gravity_utils::error::GravityError;
use gravity_utils::types::TransactionBatch;
use log::{info};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration};
use tonic::transport::channel::Channel;
use web30::client::Web3;

const LOOP_TIME: Duration = Duration::from_secs(5);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);
pub const GRAVITY_NODE_GRPC: &str = "http://gravitychain.io:9090";
pub const GRAVITY_PREFIX: &str = "gravity";
pub const ETH_NODE_RPC: &str = "https://eth.althea.net";

/// In memory store of gravity state used to serve rpc requests
#[derive(Debug, Default)]
pub struct GravityInfo {
    /// Pending transactions from Gravity Bridge to Ethereum
    pending_tx: Vec<BatchFees>,
    pending_batches: Vec<TransactionBatch>,
    attestations: Vec<Attestation>,
}

/// In memory store of Ethereum state used to serve rpc requests
#[derive(Debug, Default)]
pub struct EthInfo {}

lazy_static! {
    static ref GRAVITY_INFO: Arc<RwLock<GravityInfo>> =
        Arc::new(RwLock::new(GravityInfo::default()));
    static ref ETH_INFO: Arc<RwLock<EthInfo>> = Arc::new(RwLock::new(EthInfo::default()));
}

pub fn blockchain_info_thread() {
    info!("Starting blockchain watcher");
    let web30 = Web3::new(ETH_NODE_RPC, REQUEST_TIMEOUT);
    let contact = Contact::new(GRAVITY_NODE_GRPC, REQUEST_TIMEOUT, GRAVITY_PREFIX).unwrap();

    thread::spawn(move || loop {
        let runner = System::new();
        runner.block_on(async move {
            // since we're rebuilding the async env every loop iteration we need to re-init this
            let mut grpc_client = GravityQueryClient::connect(GRAVITY_NODE_GRPC)
                .await
                .unwrap();
        });
        thread::sleep(LOOP_TIME);
    });
}

async fn get_gravity_info(
    contact: &Contact,
    grpc_client: &mut GravityQueryClient<Channel>,
) -> Result<GravityInfo, GravityError> {
    let pending_tx = get_pending_batch_fees(grpc_client).await?.batch_fees;
    let pending_batches = get_latest_transaction_batches(grpc_client).await?;
    let attestations = get_attestations(grpc_client, None).await?;

    Ok(GravityInfo {
        pending_tx,
        pending_batches,
        attestations,
    })
}


async fn get_eth_info() -> Option<EthInfo> {
    None
}
