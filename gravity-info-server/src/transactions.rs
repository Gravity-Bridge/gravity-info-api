use cosmos_sdk_proto_althea::{
    cosmos::tx::v1beta1::{TxBody, TxRaw},
};

use log::{info, error};
use env_logger;
use rocksdb::{Options, DB};
use gravity_proto::gravity::MsgSendToEth;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use serde::{Serialize, Deserialize};

use deep_space::{
    client::Contact,
    utils::decode_any,
};

use futures::future::join_all;
use lazy_static::lazy_static;
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

lazy_static! {
    static ref COUNTER: Arc<RwLock<Counters>> = Arc::new(RwLock::new(Counters {
        blocks: 0,
        transactions: 0,
        msgs: 0
    }));
}

pub struct Counters {
    blocks: u64,
    transactions: u64,
    msgs: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CustomMsgSendToEth {
    sender: String,
    eth_dest: String,
    amount: Vec<CustomCoin>,
    bridge_fee: Vec<CustomCoin>,
    chain_fee: Vec<CustomCoin>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CustomCoin {
    denom: String,
    amount: String,
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub tx_hash: String,
    pub data: serde_json::Value,
}

impl From<&MsgSendToEth> for CustomMsgSendToEth {
    fn from(msg: &MsgSendToEth) -> Self {
        CustomMsgSendToEth {
            sender: msg.sender.clone(),
            eth_dest: msg.eth_dest.clone(),
            amount: msg
                .amount
                .iter()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .collect(),
                bridge_fee: msg
                .amount
                .iter()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .collect(),
                chain_fee: msg
                .amount
                .iter()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .collect(),
        }
    }
}

const TIMEOUT: Duration = Duration::from_secs(5);
/// finds earliest available block using binary search, keep in mind this cosmos
/// node will not have history from chain halt upgrades and could be state synced
/// and missing history before the state sync
/// Iterative implementation due to the limitations of async recursion in rust.
async fn get_earliest_block(contact: &Contact, mut start: u64, mut end: u64) -> u64 {
    while start <= end {
        let mid = start + (end - start) / 2;
        let mid_block = contact.get_block(mid).await;
        if let Ok(Some(_)) = mid_block {
            end = mid - 1;
        } else {
            start = mid + 1;
        }
    }
    // off by one error correction fix bounds logic up top
    start + 1
}

async fn search(contact: &Contact, start: u64, end: u64, db: &DB) {
    let blocks = contact.get_block_range(start, end).await.unwrap();

    let mut tx_counter = 0;
    let mut msg_counter = 0;
    let blocks_len = blocks.len() as u64;
    let mut current_block = start;

    for block in blocks {
        let block = block.unwrap();

        for tx in block.data.unwrap().txs {
            let raw_tx_any = prost_types::Any {
                type_url: "/cosmos.tx.v1beta1.TxRaw".to_string(),
                value: tx,
            };
            let tx_raw: TxRaw = decode_any(raw_tx_any).unwrap();
            let tx_hash = hex::encode(sha256::digest_bytes(&tx_raw.body_bytes));
            let body_any = prost_types::Any {
                type_url: "/cosmos.tx.v1beta1.TxBody".to_string(),
                value: tx_raw.body_bytes,
            };
            let tx_body: TxBody = decode_any(body_any).unwrap();

            let mut has_msg_send_to_eth = false;
            for message in tx_body.messages {
                if message.type_url == "/gravity.v1.MsgSendToEth" {
                    has_msg_send_to_eth = true;
                    msg_counter += 1;
            
                    let msg_send_to_eth_any = prost_types::Any {
                        type_url: "/gravity.v1.MsgSendToEth".to_string(),
                        value: message.value,
                    };
                    let msg_send_to_eth: Result<MsgSendToEth, _> = decode_any(msg_send_to_eth_any);
            
                    if let Ok(msg_send_to_eth) = msg_send_to_eth {
                        let custom_msg_send_to_eth = CustomMsgSendToEth::from(&msg_send_to_eth);
                        let key = format!("msgSendToEth_{}", tx_hash);
                        save_msg_send_to_eth(db, &key, &custom_msg_send_to_eth);
                    }
                }
            }
            if has_msg_send_to_eth {
                tx_counter += 1;
            }
        }
        current_block += 1;
    }
    let mut c = COUNTER.write().unwrap();
    c.blocks += blocks_len;
    c.transactions += tx_counter;
    c.msgs += msg_counter;
    println!("Finished processing blocks. Total blocks: {}, Total transactions: {}, Total MsgSendToEth messages: {}", c.blocks, c.transactions, c.msgs);
}


pub fn transactions(api_db: web::Data<Arc<DB>>, db: Arc<DB>, db_options: &Options) -> tokio::task::JoinHandle<()> {
    info!("Starting downloading & parsing transactions");
    tokio::spawn(async move {
    let contact = Contact::new("http://gravity-grpc.polkachu.com:14290", TIMEOUT, "gravity")
        .expect("invalid url");

    let status = contact
        .get_chain_status()
        .await
        .expect("Failed to get chain status, grpc error");

    // get the latest block this node has
    let latest_block = match status {
        deep_space::client::ChainStatus::Moving { block_height } => block_height,
        _ => panic!("Node is not synced or not running"),
    };

    // now we find the earliest block this node has via binary search, we could just read it from
    // the error message you get when requesting an earlier block, but this was more fun
    let earliest_block = get_earliest_block(&contact, 0, latest_block).await;
    info!(
        "This node has {} blocks to download, downloading to database",
        latest_block - earliest_block
    );
    let start = Instant::now();


    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

//Timestamp for downloading blocks to database
    let should_download = match load_last_download_timestamp(&db) {
        Some(timestamp) => now - timestamp > 86400,
        None => true,
    };

    if should_download {

    const BATCH_SIZE: u64 = 500;
    const EXECUTE_SIZE: usize = 200;
    let mut pos = earliest_block;
    let mut futures = Vec::new();
    while pos < latest_block {
        let start = pos;
        let end = if latest_block - pos > BATCH_SIZE {
            pos += BATCH_SIZE;
            pos
        } else {
            pos = latest_block;
            latest_block
        };
        let fut = search(&contact, start, end, &db);
        futures.push(fut);
    }

    let mut futures = futures.into_iter();

    let mut buf = Vec::new();
    while let Some(fut) = futures.next() {
        if buf.len() < EXECUTE_SIZE {
            buf.push(fut);
        } else {
            let _ = join_all(buf).await;
            println!("Completed batch of {} blocks", BATCH_SIZE * EXECUTE_SIZE as u64);
            buf = Vec::new();
        }
    }
    let _ = join_all(buf).await;

    let counter = COUNTER.read().unwrap();
    info!(
        "Successfully downloaded {} blocks and {} tx containing {} messages in {} seconds",
        counter.blocks,
        counter.transactions,
        counter.msgs,
        start.elapsed().as_secs()
    );
    save_last_download_timestamp(&db, now);
} else {
    info!("Database exists and is less than 1 day old");
}

}
    )}



pub fn save_msg_send_to_eth(db: &DB, key: &str, data: &CustomMsgSendToEth) {
    let data_json = serde_json::to_string(data).unwrap();
    db.put(key.as_bytes(), data_json.as_bytes()).unwrap();
}

// Load the MsgSendToEth transaction
pub fn load_msg_send_to_eth(db: &DB, key: &str) -> Option<CustomMsgSendToEth> {
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| serde_json::from_slice::<CustomMsgSendToEth>(&bytes).unwrap())
}

fn save_last_download_timestamp(db: &DB, timestamp: u64) {
    let key = "last_download_timestamp";
    db.put(key.as_bytes(), timestamp.to_string().as_bytes()).unwrap();
}

fn load_last_download_timestamp(db: &DB) -> Option<u64> {
    let key = "last_download_timestamp";
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| String::from_utf8_lossy(&bytes).parse::<u64>().unwrap())
}