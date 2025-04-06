use actix_rt::System;
use actix_web::{web, HttpResponse, Responder};
use clarity::utils::bytes_to_hex_str;
use clarity::{Address as EthAddress, Uint256};
use cosmos_gravity::query::{
    get_gravity_params, get_latest_transaction_batches, get_transaction_batch_signatures,
};
use ethereum_gravity::message_signatures::encode_tx_batch_confirm_hashed;
use ethereum_gravity::submit_batch::encode_batch_payload;
use gravity_proto::gravity::v1::query_client::QueryClient as GravityQueryClient;
use gravity_utils::types::TransactionBatch;
use log::{error, info};
use relayer::find_latest_valset::find_latest_valset;
use rocksdb::DB;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use web30::client::Web3;

use crate::gravity_info::{ETH_NODE_RPC, GRAVITY_NODE_GRPC, REQUEST_TIMEOUT};
use crate::transactions::database::{load_last_valset, save_last_valset};

#[derive(Debug)]
pub enum BatchRelayError {
    ServerError(String),
    BadRequest(String),
}

pub async fn generate_batch_tx_responder(
    batch_nonce: u64,
    db: web::Data<Arc<DB>>,
) -> impl Responder {
    let db = db.get_ref().clone();
    let res = generate_raw_batch_tx(batch_nonce, db).await;
    match res {
        Ok(payload) => HttpResponse::Ok().json(bytes_to_hex_str(&payload)),
        Err(BatchRelayError::ServerError(e)) => HttpResponse::InternalServerError().json(e),
        Err(BatchRelayError::BadRequest(e)) => HttpResponse::BadRequest().json(e),
    }
}

pub async fn generate_raw_batch_tx(
    batch_nonce: u64,
    db: Arc<DB>,
) -> Result<Vec<u8>, BatchRelayError> {
    let web3 = Web3::new(ETH_NODE_RPC, REQUEST_TIMEOUT);
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
        Err(_) => {
            return Err(BatchRelayError::ServerError(
                "Failed to get gravity params!".to_string(),
            ))
        }
    };

    // find the target batch and check that it's not timed out
    let latest_eth_height = match web3.eth_block_number().await {
        Ok(bn) => bn,
        Err(_) => {
            return Err(BatchRelayError::ServerError(
                "Failed to get latest eth height".to_string(),
            ))
        }
    };
    let latest_batches = match get_latest_transaction_batches(&mut grpc).await {
        Ok(v) => v,
        Err(_) => {
            return Err(BatchRelayError::ServerError(
                "Failed to get batches!".to_string(),
            ))
        }
    };
    let mut target_batch: Option<TransactionBatch> = None;
    for current_batch in latest_batches {
        if current_batch.nonce == batch_nonce {
            if Uint256::from(current_batch.batch_timeout) < latest_eth_height {
                return Err(BatchRelayError::BadRequest(
                    "Batch has timed out!".to_string(),
                ));
            }
            target_batch = Some(current_batch);
            break;
        }
    }
    let target_batch = match target_batch {
        Some(b) => b,

        None => {
            return Err(BatchRelayError::BadRequest(
                "Batch nonce not found!".to_string(),
            ))
        }
    };

    let sigs = get_transaction_batch_signatures(
        &mut grpc,
        target_batch.nonce,
        target_batch.token_contract,
    )
    .await
    .expect("Failed to get sigs for batch!");
    if sigs.is_empty() {
        return Err(BatchRelayError::ServerError(
            "Failed to get sigs for batch".to_string(),
        ));
    }

    let current_valset = match load_last_valset(&db) {
        Some(v) => v,
        None => {
            return Err(BatchRelayError::ServerError(
                "Failed to load last valset from db".to_string(),
            ))
        }
    };

    // this checks that the signatures for the batch are actually possible to submit to the chain
    let hash = encode_tx_batch_confirm_hashed(params.gravity_id.clone(), target_batch.clone());

    if let Err(e) = current_valset.order_sigs(&hash, &sigs, true) {
        error!("Current validator set is not valid to relay this batch, a validator set update must be submitted!");
        error!("{:?}", e);
        return Err(BatchRelayError::ServerError(
            "sig order not valid".to_string(),
        ));
    }

    match encode_batch_payload(current_valset, &target_batch, &sigs, params.gravity_id) {
        Ok(payload) => Ok(payload),
        Err(_) => Err(BatchRelayError::ServerError(
            "Failed to encode payload!".to_string(),
        )),
    }
}

pub fn valset_update_thread(db: Arc<DB>) {
    info!("Validator set update thread started");

    thread::spawn(move || loop {
        let runner = System::new();
        let db = db.clone();
        runner.block_on(async move {
            const SLEEP_TIME: Duration = Duration::from_secs(3600);
            const ERROR_SLEEP_TIME: Duration = Duration::from_secs(60);
            let web3 = Web3::new(ETH_NODE_RPC, REQUEST_TIMEOUT);
            loop {
                let mut grpc = loop {
                    match GravityQueryClient::connect(GRAVITY_NODE_GRPC).await {
                        Ok(client) => break client,
                        Err(e) => {
                            error!("Failed to connect to the GRPC server: {:?}", e);
                            tokio::time::sleep(ERROR_SLEEP_TIME).await;
                        }
                    }
                };
                let params = match get_gravity_params(&mut grpc).await {
                    Ok(p) => p,
                    Err(_) => {
                        error!("Failed to get gravity params!");
                        thread::sleep(ERROR_SLEEP_TIME);
                        continue;
                    }
                };
                let gravity_bridge_address: EthAddress =
                    match params.bridge_ethereum_address.parse() {
                        Ok(a) => a,
                        Err(_) => {
                            error!("Failed to parse Gravity Address?");
                            thread::sleep(ERROR_SLEEP_TIME);
                            continue;
                        }
                    };
                let valset =
                    match find_latest_valset(&mut grpc, gravity_bridge_address, &web3).await {
                        Ok(v) => v,
                        Err(_) => {
                            error!("Failed to get valset for batch");
                            thread::sleep(ERROR_SLEEP_TIME);
                            continue;
                        }
                    };
                save_last_valset(&db, &valset);
                info!("Saved valset {} to db", valset.nonce);
                thread::sleep(SLEEP_TIME);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[actix_web::test]
    async fn test_batch_relay_query() {
        let start = std::time::Instant::now();
        let db = Arc::new(DB::open_default("test_db").unwrap());
        let res = generate_raw_batch_tx(38628, db).await;
        println!(
            "Got batch response {:?} in {}s",
            res.unwrap(),
            start.elapsed().as_secs()
        );
    }
}
