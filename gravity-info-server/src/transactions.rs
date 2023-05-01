use cosmos_sdk_proto_althea::{
    cosmos::tx::v1beta1::{TxBody, TxRaw},
    ibc::{applications::transfer::v1::MsgTransfer, core::client::v1::Height},
};

use log::info;
use rocksdb::{Options, DB};
use gravity_proto::gravity::MsgSendToEth;
use actix_web::web;
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
        msgs: 0,
        ibc_msgs: 0,
        send_eth_msgs: 0
    }));
}

pub struct Counters {
    blocks: u64,
    transactions: u64,
    msgs: u64,
    ibc_msgs: u64,
    send_eth_msgs: u64,
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
pub struct CustomMsgTransfer {
    source_port: String,
    source_channel: String,
    token: Vec<CustomCoin>,
    sender: String,
    receiver: String,
    timeout_height: Option<CustomHeight>,
    timeout_timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CustomHeight {
    pub revision_number: u64,
    pub revision_height: u64,
}

impl From<&Height> for CustomHeight {
    fn from(height: &Height) -> Self {
        CustomHeight {
            revision_number: height.revision_number,
            revision_height: height.revision_height,
        }
    }
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
                .as_ref()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .into_iter()
                .collect(),
            bridge_fee: msg
                .bridge_fee
                .as_ref()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .into_iter()
                .collect(),
            chain_fee: msg
                .chain_fee
                .as_ref()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .into_iter()
                .collect(),
        }
    }
}

impl From<&MsgTransfer> for CustomMsgTransfer {
    fn from(msg: &MsgTransfer) -> Self {
        CustomMsgTransfer {
            source_port: msg.source_port.clone(),
            source_channel: msg.source_channel.clone(),
            token: msg
                .token
                .as_ref()
                .map(|coin| CustomCoin {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                })
                .into_iter()
                .collect(),
            sender: msg.sender.clone(),
            receiver: msg.receiver.clone(),
            timeout_height: msg.timeout_height.as_ref().map(|height| CustomHeight::from(height)),
            timeout_timestamp: msg.timeout_timestamp,
        }
    }
}

const DEVELOPMENT: bool = cfg!(feature = "development");
const SSL: bool = !DEVELOPMENT;
const DOMAIN: &str = if cfg!(test) || DEVELOPMENT {
    "gravity-grpc.polkachu.com"
} else {
    "info.gravitychain.io"
};
const PORT: u16 = if cfg!(test) || DEVELOPMENT {
    14290
} else {
    9000
};

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

// currently only loads MsgSendToEth messages then sends the data from the transactions to the DB
async fn search(contact: &Contact, start: u64, end: u64, db: &DB) {
    let blocks = contact.get_block_range(start, end).await.unwrap();

    let mut tx_counter = 0;
    let mut msg_counter = 0;
    let mut ibc_transfer_counter = 0;
    let mut send_eth_counter = 0;
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
            let tx_hash = sha256::digest_bytes(&tx_raw.body_bytes).to_uppercase();
            let body_any = prost_types::Any {
                type_url: "/cosmos.tx.v1beta1.TxBody".to_string(),
                value: tx_raw.body_bytes,
            };
            let tx_body: TxBody = decode_any(body_any).unwrap();

            let mut has_msg_send_to_eth = false;
            let mut has_msg_ibc_transfer = false;

            // tx sorting
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
                } else if message.type_url == "/ibc.applications.transfer.v1.MsgTransfer" {
                    has_msg_ibc_transfer = true;
                    msg_counter += 1;

                    let msg_ibc_transfer_any = prost_types::Any {
                        type_url: "/ibc.applications.transfer.v1.MsgTransfer".to_string(),
                        value: message.value,
                    };
                    let msg_ibc_transfer: Result<MsgTransfer, _> = decode_any(msg_ibc_transfer_any);

                    if let Ok(msg_ibc_transfer) = msg_ibc_transfer {
                        let custom_ibc_transfer = CustomMsgTransfer::from(&msg_ibc_transfer);
                        let key = format!("msgIbcTransfer_{}", tx_hash);
                        save_msg_ibc_transfer(db, &key, &custom_ibc_transfer);
                    }
                }
            }

            if has_msg_send_to_eth {
                tx_counter += 1;
                send_eth_counter += 1;

            }
            if has_msg_ibc_transfer {
                tx_counter += 1;
                ibc_transfer_counter += 1;
            }
        }
        current_block += 1;
    }
    let mut c = COUNTER.write().unwrap();
    c.blocks += blocks_len;
    c.transactions += tx_counter;
    c.msgs += msg_counter;
    c.ibc_msgs += ibc_transfer_counter;
    c.send_eth_msgs += send_eth_counter;
    info!(
        "Parsed {} blocks, {} transactions, {} ibc messages, {} send to eth messages",
        blocks_len, tx_counter, ibc_transfer_counter, send_eth_counter
    );
}

pub fn transactions(api_db: web::Data<Arc<DB>>, db: Arc<DB>, db_options: &Options) -> tokio::task::JoinHandle<()> {
    info!("Started downloading & parsing transactions");
    tokio::spawn(async move {
    let url = format!("http://{}:{}", DOMAIN, PORT);
    let contact = Contact::new(&url, TIMEOUT, "gravity")
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
            info!("Completed batch of {} blocks", BATCH_SIZE * EXECUTE_SIZE as u64);
            buf = Vec::new();
        }
    }
    let _ = join_all(buf).await;

    let counter = COUNTER.read().unwrap();
    info!(
        "Successfully downloaded {} blocks and {} tx containing {} send_to_eth msgs and {} ibc_transfer msgs in {} seconds",
        counter.blocks,
        counter.transactions,
        counter.send_eth_msgs,
        counter.ibc_msgs,
        start.elapsed().as_secs()
    );
    save_last_download_timestamp(&db, now);
} else {
    info!("Database exists and is less than 1 day old");
}

}
    )}


//saves serialized MsgSendToEth transaction to database
pub fn save_msg_send_to_eth(db: &DB, key: &str, data: &CustomMsgSendToEth) {
    let data_json = serde_json::to_string(data).unwrap();
    db.put(key.as_bytes(), data_json.as_bytes()).unwrap();
}

// Load & deseralize the MsgSendToEth transaction
pub fn load_msg_send_to_eth(db: &DB, key: &str) -> Option<CustomMsgSendToEth> {
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| serde_json::from_slice::<CustomMsgSendToEth>(&bytes).unwrap())
}

pub fn save_msg_ibc_transfer(db: &DB, key: &str, data: &CustomMsgTransfer) {
    let data_json = serde_json::to_string(data).unwrap();
    db.put(key.as_bytes(), data_json.as_bytes()).unwrap();
}

// Load & deseralize the MsgSendToEth transaction
pub fn load_msg_ibc_transfer(db: &DB, key: &str) -> Option<CustomMsgTransfer> {
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| serde_json::from_slice::<CustomMsgTransfer>(&bytes).unwrap())
}

// Timestamp functions
fn save_last_download_timestamp(db: &DB, timestamp: u64) {
    let key = "last_download_timestamp";
    db.put(key.as_bytes(), timestamp.to_string().as_bytes()).unwrap();
}

fn load_last_download_timestamp(db: &DB) -> Option<u64> {
    let key = "last_download_timestamp";
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| String::from_utf8_lossy(&bytes).parse::<u64>().unwrap())
}