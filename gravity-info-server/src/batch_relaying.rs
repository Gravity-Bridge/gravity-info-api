use actix_web::{HttpResponse, Responder};
use clarity::utils::bytes_to_hex_str;
use clarity::{Address as EthAddress, Uint256};
use cosmos_gravity::query::{
    get_gravity_params, get_latest_transaction_batches, get_transaction_batch_signatures,
};
use deep_space::Contact;
use ethereum_gravity::message_signatures::encode_tx_batch_confirm_hashed;
use ethereum_gravity::submit_batch::encode_batch_payload;
use gravity_proto::gravity::query_client::QueryClient as GravityQueryClient;
use gravity_utils::types::TransactionBatch;
use log::error;
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

    if let Err(e) = current_valset.order_sigs(&hash, &sigs, true) {
        error!("Current validator set is not valid to relay this batch, a validator set update must be submitted!");
        error!("{:?}", e);
        return HttpResponse::InternalServerError().json("sig order not valid");
    }

    match encode_batch_payload(current_valset, &target_batch, &sigs, params.gravity_id) {
        Ok(payload) => HttpResponse::Ok().json(bytes_to_hex_str(&payload)),
        Err(_) => HttpResponse::InternalServerError().json("Failed to encode payload!"),
    }
}
