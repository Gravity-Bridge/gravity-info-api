use crate::transactions::database::CustomCoin;
use crate::transactions::database::{ApiResponse, CustomMsgSendToEth, CustomMsgTransfer};

use actix_web::Responder;
use actix_web::{web, HttpResponse};
use chrono::{DateTime, Datelike, Local, NaiveDateTime, Utc};

use log::error;

use rocksdb::DB;

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Serialize)]
struct TimeFrameData {
    time_frames: Vec<TimeFrame>,
}

#[derive(Debug, Serialize)]
struct TimeFrame {
    period: String,
    bridge_fee_totals: HashMap<String, u128>,
    chain_fee_totals: HashMap<String, u128>,
}

#[derive(Serialize)]
struct BlockTransactions {
    block_number: u64,
    transactions: Vec<ApiResponse>,
    formatted_date: String,
}

type BlockData = (String, Vec<ApiResponse>);

fn process_fee(fee: Vec<CustomCoin>, totals: &HashMap<String, u128>) -> HashMap<String, u128> {
    let mut new_totals = totals.clone();
    for custom_coin in fee {
        let decimal_value = custom_coin.amount.parse::<u128>().unwrap();
        let denom = custom_coin.denom.clone();
        *new_totals.entry(denom).or_default() += decimal_value;
    }
    new_totals
}

pub async fn get_all_msg_send_to_eth_transactions(db: web::Data<Arc<DB>>) -> impl Responder {
    let mut response_data: HashMap<u64, BlockData> = HashMap::new();

    let iterator = db.iterator(rocksdb::IteratorMode::Start);

    for item in iterator {
        match item {
            Ok((key, value)) => {
                let key_str = String::from_utf8_lossy(&key);
                let key_parts: Vec<&str> = key_str.split(':').collect();
                if key_parts.len() == 4 && key_parts[1] == "msgSendToEth" {
                    let msg_send_to_eth: CustomMsgSendToEth =
                        serde_json::from_slice(&value).unwrap();
                    let block_number = key_parts[0].parse::<u64>().unwrap();

                    let timestamp = key_parts[2].parse::<i64>().unwrap();

                    // Convert timestamp to Option<NaiveDateTime>
                    let naive_opt = NaiveDateTime::from_timestamp_opt(timestamp, 0);

                    let mut _datetime_utc: Option<DateTime<Utc>> = None;

                    if let Some(naive_datetime) = naive_opt {
                        // Convert Option<NaiveDateTime> to DateTime
                        _datetime_utc = Some(DateTime::<Utc>::from_utc(naive_datetime, Utc));
                    } else {
                        error!("Invalid timestamp: {}", timestamp);
                        continue; // skip this iteration if timestamp is invalid
                    }

                    let datetime_utc = _datetime_utc.unwrap(); // we can safely unwrap because of the `continue` above

                    let datetime_local: DateTime<Local> = datetime_utc.into();

                    // Extract month, day, and year
                    let month = datetime_local.month();
                    let day = datetime_local.day();
                    let year = datetime_local.year();

                    // Format the date string
                    let formatted_date = format!("{:02}-{:02}-{}", month, day, year);
                    let api_response = ApiResponse {
                        tx_hash: key_parts[3].to_string(),
                        data: serde_json::to_value(&msg_send_to_eth).unwrap(),
                    };

                    response_data
                        .entry(block_number)
                        .or_insert((formatted_date, Vec::new()))
                        .1
                        .push(api_response);
                }
            }
            Err(err) => {
                error!("RocksDB iterator error: {}", err);
            }
        }
    }

    // Converting the HashMap to a Vec and sorting it by block number
    let mut response_data: Vec<_> = response_data.into_iter().collect();
    response_data.sort_by(|a, b| a.0.cmp(&b.0));

    // Convert Vec of tuples into Vec of BlockTransactions
    let response_data: Vec<_> = response_data
        .into_iter()
        .map(
            |(block_number, (formatted_date, transactions))| BlockTransactions {
                block_number,
                formatted_date,
                transactions,
            },
        )
        .collect();

    HttpResponse::Ok().json(response_data)
}

pub async fn get_all_msg_ibc_transfer_transactions(db: web::Data<Arc<DB>>) -> impl Responder {
    let mut response_data: HashMap<u64, BlockData> = HashMap::new();

    let iterator = db.iterator(rocksdb::IteratorMode::Start);

    for item in iterator {
        match item {
            Ok((key, value)) => {
                let key_str = String::from_utf8_lossy(&key);
                let key_parts: Vec<&str> = key_str.split(':').collect();
                if key_parts.len() == 4 && key_parts[1] == "msgIbcTransfer" {
                    let msg_ibc_transfer: CustomMsgTransfer =
                        serde_json::from_slice(&value).unwrap();
                    let block_number = key_parts[0].parse::<u64>().unwrap();

                    let timestamp = key_parts[2].parse::<i64>().unwrap();

                    // Convert timestamp to Option<NaiveDateTime>
                    let naive_opt = NaiveDateTime::from_timestamp_opt(timestamp, 0);

                    let mut _datetime_utc: Option<DateTime<Utc>> = None;

                    if let Some(naive_datetime) = naive_opt {
                        // Convert Option<NaiveDateTime> to DateTime
                        _datetime_utc = Some(DateTime::<Utc>::from_utc(naive_datetime, Utc));
                    } else {
                        error!("Invalid timestamp: {}", timestamp);
                        continue; // skip this iteration if timestamp is invalid
                    }

                    let datetime_utc = _datetime_utc.unwrap(); // we can safely unwrap because of the `continue` above

                    let datetime_local: DateTime<Local> = datetime_utc.into();

                    // Extract month, day, and year
                    let month = datetime_local.month();
                    let day = datetime_local.day();
                    let year = datetime_local.year();

                    // Format the date string
                    let formatted_date = format!("{:02}-{:02}-{}", month, day, year);
                    let api_response = ApiResponse {
                        tx_hash: key_parts[3].to_string(),
                        data: serde_json::to_value(&msg_ibc_transfer).unwrap(),
                    };

                    response_data
                        .entry(block_number)
                        .or_insert((formatted_date, Vec::new()))
                        .1
                        .push(api_response);
                }
            }
            Err(err) => {
                error!("RocksDB iterator error: {}", err);
            }
        }
    }

    // Converting the HashMap to a Vec and sorting it by block number
    let mut response_data: Vec<_> = response_data.into_iter().collect();
    response_data.sort_by(|a, b| a.0.cmp(&b.0));

    // Convert Vec of tuples into Vec of BlockTransactions
    let response_data: Vec<_> = response_data
        .into_iter()
        .map(
            |(block_number, (formatted_date, transactions))| BlockTransactions {
                block_number,
                formatted_date,
                transactions,
            },
        )
        .collect();

    HttpResponse::Ok().json(response_data)
}

pub async fn get_send_to_eth_transaction_totals(db: web::Data<Arc<DB>>) -> impl Responder {
    // Define the time frame duration in seconds
    const ONE_DAY: u64 = 24 * 60 * 60;
    const SEVEN_DAYS: u64 = 7 * ONE_DAY;
    const THIRTY_DAYS: u64 = 30 * ONE_DAY;
    const ONE_YEAR: u64 = 365 * ONE_DAY;

    let mut bridge_fee_totals_1day: HashMap<String, u128> = HashMap::new();
    let mut chain_fee_totals_1day: HashMap<String, u128> = HashMap::new();

    let mut bridge_fee_totals_7days: HashMap<String, u128> = HashMap::new();
    let mut chain_fee_totals_7days: HashMap<String, u128> = HashMap::new();

    let mut bridge_fee_totals_30days: HashMap<String, u128> = HashMap::new();
    let mut chain_fee_totals_30days: HashMap<String, u128> = HashMap::new();

    let mut bridge_fee_totals_1year: HashMap<String, u128> = HashMap::new();
    let mut chain_fee_totals_1year: HashMap<String, u128> = HashMap::new();

    let mut bridge_fee_totals_alltime: HashMap<String, u128> = HashMap::new();
    let mut chain_fee_totals_alltime: HashMap<String, u128> = HashMap::new();

    let iterator = db.iterator(rocksdb::IteratorMode::Start);

    for item in iterator {
        match item {
            Ok((key, value)) => {
                let key_str = String::from_utf8_lossy(&key);
                let key_parts: Vec<&str> = key_str.split(':').collect();
                if key_parts.len() == 4 && key_parts[1] == "msgSendToEth" {
                    let msg_send_to_eth: CustomMsgSendToEth =
                        serde_json::from_slice(&value).unwrap();
                    let timestamp = key_parts[2].parse::<i64>().unwrap();

                    let bridge_fee = msg_send_to_eth.bridge_fee.clone();
                    let chain_fee = msg_send_to_eth.chain_fee.clone();

                    bridge_fee_totals_alltime =
                        process_fee(bridge_fee.clone(), &bridge_fee_totals_alltime);
                    chain_fee_totals_alltime =
                        process_fee(chain_fee.clone(), &chain_fee_totals_alltime);

                    // process data
                    if timestamp
                        >= (Utc::now() - chrono::Duration::seconds(ONE_DAY as i64)).timestamp()
                    {
                        // 1-day time frame
                        bridge_fee_totals_1day =
                            process_fee(bridge_fee.clone(), &bridge_fee_totals_1day);
                        chain_fee_totals_1day =
                            process_fee(chain_fee.clone(), &chain_fee_totals_1day);
                    }
                    if timestamp
                        >= (Utc::now() - chrono::Duration::seconds(SEVEN_DAYS as i64)).timestamp()
                    {
                        // 7-day time frame
                        bridge_fee_totals_7days =
                            process_fee(bridge_fee.clone(), &bridge_fee_totals_7days);
                        chain_fee_totals_7days =
                            process_fee(chain_fee.clone(), &chain_fee_totals_7days);
                    }
                    if timestamp
                        >= (Utc::now() - chrono::Duration::seconds(THIRTY_DAYS as i64)).timestamp()
                    {
                        // 30-day time frame
                        bridge_fee_totals_30days =
                            process_fee(bridge_fee.clone(), &bridge_fee_totals_30days);
                        chain_fee_totals_30days =
                            process_fee(chain_fee.clone(), &chain_fee_totals_30days);
                    }
                    if timestamp
                        >= (Utc::now() - chrono::Duration::seconds(ONE_YEAR as i64)).timestamp()
                    {
                        // 1-year time frame
                        bridge_fee_totals_1year =
                            process_fee(bridge_fee.clone(), &bridge_fee_totals_1year);
                        chain_fee_totals_1year =
                            process_fee(chain_fee.clone(), &chain_fee_totals_1year);
                    }
                }
            }
            Err(err) => {
                error!("RocksDB iterator error: {}", err);
            }
        }
    }

    let response_data = TimeFrameData {
        time_frames: vec![
            TimeFrame {
                period: "1 day".to_string(),
                bridge_fee_totals: bridge_fee_totals_1day,
                chain_fee_totals: chain_fee_totals_1day,
            },
            TimeFrame {
                period: "7 days".to_string(),
                bridge_fee_totals: bridge_fee_totals_7days,
                chain_fee_totals: chain_fee_totals_7days,
            },
            TimeFrame {
                period: "30 days".to_string(),
                bridge_fee_totals: bridge_fee_totals_30days,
                chain_fee_totals: chain_fee_totals_30days,
            },
            TimeFrame {
                period: "1 year".to_string(),
                bridge_fee_totals: bridge_fee_totals_1year,
                chain_fee_totals: chain_fee_totals_1year,
            },
            TimeFrame {
                period: "All time".to_string(),
                bridge_fee_totals: bridge_fee_totals_alltime,
                chain_fee_totals: chain_fee_totals_alltime,
            },
        ],
    };

    HttpResponse::Ok().json(response_data)
}
