use crate::gravity_info::{GRAVITY_NODE_GRPC, GRAVITY_PREFIX, REQUEST_TIMEOUT};
use actix_rt::System;
use cosmos_sdk_proto_althea::{
    cosmos::tx::v1beta1::{TxBody, TxRaw},
    ibc::core::channel::v1::{Acknowledgement, MsgAcknowledgement},
    ibc::{applications::transfer::v1::MsgTransfer, core::client::v1::Height},
};
use deep_space::{client::Contact, utils::decode_any};
use futures::future::join_all;
use gravity_proto::gravity::MsgSendToEth;
use lazy_static::lazy_static;
use log::{error, info};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Instant,
};
use tokio::time::sleep;

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
    pub amount: Vec<CustomCoin>,
    pub bridge_fee: Vec<CustomCoin>,
    pub chain_fee: Vec<CustomCoin>,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CustomCoin {
    pub denom: String,
    pub amount: String,
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
            timeout_height: msg.timeout_height.as_ref().map(CustomHeight::from),
            timeout_timestamp: msg.timeout_timestamp,
        }
    }
}

const MAX_RETRIES: usize = 5;

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

// Loads sendToEth & MsgTransfer messages from grpc endpoint & downlaods to DB
async fn search(contact: &Contact, start: u64, end: u64, db: &DB) {
    info!("Searching block range {} => {}", start, end);
    let mut current_start = start;
    let retries = AtomicUsize::new(0);

    loop {
        let blocks_result = contact.get_block_range(current_start, end).await;

        let blocks = match blocks_result {
            Ok(result) => {
                retries.store(0, Ordering::Relaxed);
                result
            }
            Err(e) => {
                let current_retries = retries.fetch_add(1, Ordering::Relaxed);
                if current_retries >= MAX_RETRIES {
                    error!("Error getting block range: {:?}, exceeded max retries", e);
                    break;
                } else {
                    error!("Error getting block range: {:?}, retrying", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        };

        if blocks.is_empty() {
            break;
        }

        // gets the last block that was successfully fetched to be referenced
        // in case of grpc error
        let last_block_height = blocks
            .last()
            .unwrap()
            .as_ref()
            .unwrap()
            .header
            .as_ref()
            .unwrap()
            .height;

        // counters for transactions, messages, blocks & tx types
        let mut tx_counter = 0;
        let mut msg_counter = 0;
        let mut ibc_transfer_counter = 0;
        let mut send_eth_counter = 0;
        let blocks_len = blocks.len() as u64;

        const IBC_RECV: &str = "/ibc.core.channel.v1.MsgAcknowledgement";
        const IBC_ACK: &str = "/ibc.core.channel.v1.Acknowledgement";
        const MSG_TRANSFER: &str = "/ibc.applications.transfer.v1.MsgTransfer";
        const MSG_SEND_TO_ETH: &str = "/gravity.v1.MsgSendToEth";

        for block in blocks.into_iter() {
            let block = block.unwrap();
            // Get the block number
            let block_number = block.header.as_ref().unwrap().height;

            // tx fetching
            for tx in block.data.unwrap().txs {
                let raw_tx_any = prost_types::Any {
                    type_url: "/cosmos.tx.v1beta1.TxRaw".to_string(),
                    value: tx,
                };
                let tx_raw: TxRaw = decode_any(raw_tx_any.clone()).unwrap();
                let value_ref: &[u8] = raw_tx_any.value.as_ref();
                let tx_hash = sha256::digest(value_ref).to_uppercase();
                let body_any = prost_types::Any {
                    type_url: "/cosmos.tx.v1beta1.TxBody".to_string(),
                    value: tx_raw.body_bytes,
                };
                let tx_body: TxBody = decode_any(body_any).unwrap();

                let mut has_msg_send_to_eth = false;
                let mut has_msg_ibc_transfer = false;

                // tx sorting
                for message in tx_body.messages {
                    if message.type_url == MSG_SEND_TO_ETH {
                        has_msg_send_to_eth = true;
                        msg_counter += 1;

                        let msg_send_to_eth_any = prost_types::Any {
                            type_url: MSG_SEND_TO_ETH.to_string(),
                            value: message.value,
                        };
                        let msg_send_to_eth: Result<MsgSendToEth, _> =
                            decode_any(msg_send_to_eth_any);

                        if let Ok(msg_send_to_eth) = msg_send_to_eth {
                            let custom_msg_send_to_eth = CustomMsgSendToEth::from(&msg_send_to_eth);
                            let timestamp = block
                                .header
                                .as_ref()
                                .unwrap()
                                .time
                                .as_ref()
                                .unwrap()
                                .seconds;
                            let key = format!(
                                "{:012}:msgSendToEth:{}:{}",
                                block_number, timestamp, tx_hash
                            );
                            save_msg_send_to_eth(db, &key, &custom_msg_send_to_eth);
                        }
                    } else if message.type_url == MSG_TRANSFER {
                        has_msg_ibc_transfer = true;
                        msg_counter += 1;

                        let msg_ibc_transfer_any = prost_types::Any {
                            type_url: MSG_TRANSFER.to_string(),
                            value: message.value,
                        };
                        let msg_ibc_transfer: Result<MsgTransfer, _> =
                            decode_any(msg_ibc_transfer_any);

                        if let Ok(msg_ibc_transfer) = msg_ibc_transfer {
                            let custom_ibc_transfer = CustomMsgTransfer::from(&msg_ibc_transfer);
                            let timestamp = block
                                .header
                                .as_ref()
                                .unwrap()
                                .time
                                .as_ref()
                                .unwrap()
                                .seconds;
                            let key = format!(
                                "{:012}:msgIbcTransfer:{}:{}",
                                block_number, timestamp, tx_hash
                            );
                            save_msg_ibc_transfer(db, &key, &custom_ibc_transfer);
                        }
                    } else if message.type_url == IBC_RECV {
                        has_msg_ibc_transfer = true;
                        msg_counter += 1;

                        let msg_ibc_recv_any = prost_types::Any {
                            type_url: IBC_RECV.to_string(),
                            value: message.value,
                        };
                        let msg_ibc_recv: Result<MsgAcknowledgement, _> =
                            decode_any(msg_ibc_recv_any);

                        if let Ok(msg_ibc_recv) = msg_ibc_recv {
                            info!("Decoded message ibc recv");
                            // this is only a option becuase it's a pointer it should never be null
                            let msg_ibc_transfer_any = prost_types::Any {
                                type_url: IBC_ACK.to_string(),
                                value: msg_ibc_recv.acknowledgement,
                            };
                            let msg_ibc_transfer: Acknowledgement =
                                decode_any(msg_ibc_transfer_any).unwrap();

                            let response = match msg_ibc_transfer.response.unwrap() {
                                cosmos_sdk_proto_althea::ibc::core::channel::v1::acknowledgement::Response::Result(b) => b,
                                cosmos_sdk_proto_althea::ibc::core::channel::v1::acknowledgement::Response::Error(_) => continue,
                            };

                            let msg_ibc_transfer_any = prost_types::Any {
                                type_url: MSG_TRANSFER.to_string(),
                                value: response,
                            };
                            let msg_ibc_transfer: Result<MsgTransfer, _> =
                                decode_any(msg_ibc_transfer_any);

                            if let Ok(msg_ibc_transfer) = msg_ibc_transfer {
                                info!("Decoded transfer inside ibc_recv");
                                let custom_ibc_transfer =
                                    CustomMsgTransfer::from(&msg_ibc_transfer);
                                let timestamp = block
                                    .header
                                    .as_ref()
                                    .unwrap()
                                    .time
                                    .as_ref()
                                    .unwrap()
                                    .seconds;
                                let key = format!(
                                    "{:012}:msgIbcRecv:{}:{}",
                                    block_number, timestamp, tx_hash
                                );
                                save_msg_ibc_recv(db, &key, &custom_ibc_transfer);
                            }
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
            current_start = (last_block_height as u64) + 1;
            if current_start > end {
                break;
            }
        }
        let mut c = COUNTER.write().unwrap();
        c.blocks += blocks_len;
        c.transactions += tx_counter;
        c.msgs += msg_counter;
        c.ibc_msgs += ibc_transfer_counter;
        c.send_eth_msgs += send_eth_counter;
    }
}

pub fn transaction_info_thread(db: Arc<DB>) {
    info!("Starting transaction info thread");

    thread::spawn(move || loop {
        let runner = System::new();
        runner.block_on(async {
            match transactions(&db).await {
                Ok(_) => (),
                Err(e) => {
                    error!("Error downloading transactions: {:?}", e);
                    let mut retry_interval = Duration::from_secs(1);
                    loop {
                        info!("Retrying block download");
                        sleep(retry_interval).await;
                        match transactions(&db).await {
                            Ok(_) => break,
                            Err(e) => {
                                error!("Error in transaction download retry: {:?}", e);
                                retry_interval =
                                    if let Some(new_interval) = retry_interval.checked_mul(2) {
                                        new_interval
                                    } else {
                                        retry_interval
                                    };
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await; // Sleep for 24 hours
        });
    });
}

/// creates batches of transactions found and sorted using the search function
/// then writes them to the db
pub async fn transactions(db: &DB) -> Result<(), Box<dyn std::error::Error>> {
    info!("Started downloading & parsing transactions");
    let contact: Contact = Contact::new(GRAVITY_NODE_GRPC, REQUEST_TIMEOUT, GRAVITY_PREFIX)?;

    let mut latest_block = None;
    while latest_block.is_none() {
        match contact.get_latest_block().await {
            Ok(v) => match v {
                deep_space::client::types::LatestBlock::Latest { block }
                | deep_space::client::types::LatestBlock::Syncing { block } => {
                    latest_block = Some(block.header.unwrap().height as u64)
                }
                deep_space::client::types::LatestBlock::WaitingToStart => {
                    error!("Node is waiting to start after an upgrade, can not get latest block")
                }
            },
            Err(e) => error!("Failed to get latest block with {:?}, retrying", e),
        }
    }

    let latest_block = latest_block.expect("Node is not synced or not running");

    // now we find the earliest block this node has via binary search, we could just read it from
    // the error message you get when requesting an earlier block, but this was more fun
    let earliest_block = get_earliest_block(&contact, 0, latest_block).await;

    let earliest_block = match load_last_download_block(db) {
        Some(block) => block,
        None => earliest_block,
    };

    info!(
        "This node has {} blocks to download, downloading to database",
        latest_block - earliest_block
    );
    let start = Instant::now();

    // how many blocks to search per future
    const BATCH_SIZE: u64 = 500;
    // how many futures to execute at once
    const EXECUTE_SIZE: usize = 10;
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
        let fut = search(&contact, start, end, db);
        futures.push(fut);
    }

    let futures = futures.into_iter();

    let mut buf = Vec::new();

    for fut in futures {
        if buf.len() < EXECUTE_SIZE {
            buf.push(fut);
        } else {
            let _ = join_all(buf).await;
            info!(
                "Completed batch of {} blocks",
                BATCH_SIZE * EXECUTE_SIZE as u64
            );
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
    save_last_download_block(db, latest_block);
    Ok(())
}

//saves serialized transactions to database
pub fn save_msg_send_to_eth(db: &DB, key: &str, data: &CustomMsgSendToEth) {
    info!("Send to eth save!");
    let data_json = serde_json::to_string(data).unwrap();
    db.put(key.as_bytes(), data_json.as_bytes()).unwrap();
}

pub fn save_msg_ibc_transfer(db: &DB, key: &str, data: &CustomMsgTransfer) {
    info!("Ibc transfer save");
    let data_json = serde_json::to_string(data).unwrap();
    db.put(key.as_bytes(), data_json.as_bytes()).unwrap();
}

pub fn save_msg_ibc_recv(db: &DB, key: &str, data: &CustomMsgTransfer) {
    info!("Successful save!");
    let data_json = serde_json::to_string(data).unwrap();
    db.put(key.as_bytes(), data_json.as_bytes()).unwrap();
}

// Load & deseralize transactions
pub fn load_msg_send_to_eth(db: &DB, key: &str) -> Option<CustomMsgSendToEth> {
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| serde_json::from_slice::<CustomMsgSendToEth>(&bytes).unwrap())
}

pub fn load_msg_ibc_transfer(db: &DB, key: &str) -> Option<CustomMsgTransfer> {
    let res = db.get(key.as_bytes()).unwrap();
    res.map(|bytes| serde_json::from_slice::<CustomMsgTransfer>(&bytes).unwrap())
}

// timestamp function using downloaded blocks as a source of truth
const LAST_DOWNLOAD_BLOCK_KEY: &str = "last_download_block";

fn save_last_download_block(db: &DB, timestamp: u64) {
    db.put(
        LAST_DOWNLOAD_BLOCK_KEY.as_bytes(),
        timestamp.to_string().as_bytes(),
    )
    .unwrap();
}

fn load_last_download_block(db: &DB) -> Option<u64> {
    let res = db.get(LAST_DOWNLOAD_BLOCK_KEY.as_bytes()).unwrap();
    res.map(|bytes| String::from_utf8_lossy(&bytes).parse::<u64>().unwrap())
}
