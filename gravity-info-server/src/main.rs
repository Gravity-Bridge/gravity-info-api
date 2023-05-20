#[macro_use]
extern crate lazy_static;

pub mod gravity_info;
pub mod tls;
pub mod total_suppy;
pub mod transactions;
pub mod volume;

const DEVELOPMENT: bool = cfg!(feature = "development");
const SSL: bool = !DEVELOPMENT;
const DOMAIN: &str = if cfg!(test) || DEVELOPMENT {
    "localhost"
} else {
    "info.gravitychain.io"
};
const PORT: u16 = 9000;

use crate::gravity_info::get_erc20_metadata;
use crate::total_suppy::get_supply_info;
use crate::volume::get_volume_info;
use crate::{gravity_info::get_gravity_info, tls::*};
use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use env_logger::Env;
use gravity_info::{blockchain_info_thread, get_eth_info};
use log::{error, info};
use rocksdb::Options;
use rocksdb::DB;
use rustls::ServerConfig;
use serde::Serialize;
use std::sync::Arc;
use std::collections::HashMap;
use total_suppy::chain_total_supply_thread;
use transactions::{transaction_info_thread, ApiResponse, CustomMsgSendToEth, CustomMsgTransfer};
use volume::bridge_volume_thread;
use chrono::{DateTime, Local, NaiveDateTime, Utc, Datelike};

#[derive(Debug, Serialize)]
struct TimeFrameData {
    time_frames: Vec<TimeFrame>,
}

#[derive(Debug, Serialize)]
struct TimeFrame {
    name: String,
    totals: HashMap<String, u128>,
}

#[derive(Serialize)]
struct BlockTransactions {
    block_number: u64,
    transactions: Vec<ApiResponse>,
    formatted_date: String,
}

type BlockData = (String, Vec<ApiResponse>); 


#[get("/total_supply")]
async fn get_total_supply() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => HttpResponse::Ok().json(v.total_supply),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/total_liquid_supply")]
async fn get_total_liquid_supply() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => HttpResponse::Ok().json(v.total_liquid_supply),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/supply_info")]
async fn get_all_supply_info() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => HttpResponse::Ok().json(v),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/eth_bridge_info")]
async fn get_eth_bridge_info() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_eth_info() {
        Some(v) => HttpResponse::Ok().json(v),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/gravity_bridge_info")]
async fn get_gravity_bridge_info() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_gravity_info() {
        Some(v) => HttpResponse::Ok().json(v),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/erc20_metadata")]
async fn erc20_metadata() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_erc20_metadata() {
        Some(v) => HttpResponse::Ok().json(v),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/bridge_volume")]
async fn get_bridge_volume() -> impl Responder {
    // if we have already computed volume info return it, if not return an error
    match get_volume_info() {
        Some(v) => HttpResponse::Ok().json(v),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 20 minutes"),
    }
}

#[get("/transactions/send_to_eth")]
async fn get_all_msg_send_to_eth_transactions(db: web::Data<Arc<DB>>) -> impl Responder {
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
                    
                    // Convert timestamp to DateTime
                    let datetime_utc = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(timestamp, 0), Utc);
                    
                    // Convert DateTime to local time zone
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
        .map(|(block_number, (formatted_date, transactions))| {
            BlockTransactions {
                block_number,
                formatted_date,
                transactions,
            }
        })
        .collect();

    HttpResponse::Ok().json(response_data)
}

#[get("/transactions/ibc_transfer")]
async fn get_all_msg_ibc_transfer_transactions(db: web::Data<Arc<DB>>) -> impl Responder {
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

                    // Convert timestamp to DateTime
                    let datetime_utc = DateTime::<Utc>::from_utc(
                        NaiveDateTime::from_timestamp(timestamp, 0),
                        Utc,
                    );

                    // Convert DateTime to local time zone
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
        .map(|(block_number, (formatted_date, transactions))| {
            BlockTransactions {
                block_number,
                formatted_date,
                transactions,
            }
        })
        .collect();

    HttpResponse::Ok().json(response_data)
}

#[get("/transactions/send_to_eth/time")]
async fn get_send_to_eth_transaction_totals(db: web::Data<Arc<DB>>) -> impl Responder {
    // Define the time frame duration in seconds
    const ONE_DAY: u64 = 24 * 60 * 60;
    const SEVEN_DAYS: u64 = 7 * ONE_DAY;
    const THIRTY_DAYS: u64 = 30 * ONE_DAY;

    let mut totals_1day: HashMap<String, u128> = HashMap::new();
    let mut totals_7days: HashMap<String, u128> = HashMap::new();
    let mut totals_30days: HashMap<String, u128> = HashMap::new();

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
                    let amount = msg_send_to_eth.amount;

                    if timestamp > (Utc::now() - chrono::Duration::from_std(std::time::Duration::from_secs(ONE_DAY)).unwrap()).timestamp() {
                        // Within the 1-day time frame
                        for custom_coin in amount.iter() {
                            let decimal_value = custom_coin.amount.parse::<u128>().unwrap();
                            let denom = custom_coin.denom.clone();
                            info!("Processing CustomCoin: denom={}, amount={}", denom, custom_coin.amount);
                            *totals_1day.entry(denom).or_default() += decimal_value;
                        }
                    }
                    if timestamp > (Utc::now() - chrono::Duration::from_std(std::time::Duration::from_secs(SEVEN_DAYS)).unwrap()).timestamp() {
                        // Within the 7-day time frame
                        for custom_coin in amount.iter() {
                            let decimal_value = custom_coin.amount.parse::<u128>().unwrap();
                            let denom = custom_coin.denom.clone();
                            info!("Processing CustomCoin: denom={}, amount={}", denom, custom_coin.amount);
                            *totals_7days.entry(denom).or_default() += decimal_value;
                        }
                    }
                    if timestamp > (Utc::now() - chrono::Duration::from_std(std::time::Duration::from_secs(THIRTY_DAYS)).unwrap()).timestamp() {
                        // Within the 30-day time frame
                        for custom_coin in amount.iter() {
                            let decimal_value = custom_coin.amount.parse::<u128>().unwrap();
                            let denom = custom_coin.denom.clone();
                            info!("Processing CustomCoin: denom={}, amount={}", denom, custom_coin.amount);
                            *totals_30days.entry(denom).or_default() += decimal_value;
                        }
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
                name: "1 day".to_string(),
                totals: totals_1day,
            },
            TimeFrame {
                name: "7 days".to_string(),
                totals: totals_7days,
            },
            TimeFrame {
                name: "30 days".to_string(),
                totals: totals_30days,
            },
        ],
    };

    HttpResponse::Ok().json(response_data)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    openssl_probe::init_ssl_cert_env_vars();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    // starts a background thread for downloading transactions
    let mut db_options = Options::default();
    db_options.create_if_missing(true);
    let db = Arc::new(DB::open(&db_options, "transactions").expect("Failed to open database"));
    let api_db = web::Data::new(db.clone());
    transaction_info_thread(db.clone());
    // starts background thread for gathering into
    blockchain_info_thread();
    // starts a background thread for generating the total supply numbers
    chain_total_supply_thread();
    // starts a background thread for generating volume numbers
    bridge_volume_thread();

    let server = HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_header()
                    .allow_any_method(),
            )
            .service(get_total_supply)
            .service(get_total_liquid_supply)
            .service(get_all_supply_info)
            .service(get_eth_bridge_info)
            .service(get_gravity_bridge_info)
            .service(erc20_metadata)
            .service(get_bridge_volume)
            .app_data(api_db.clone())
            .service(get_all_msg_send_to_eth_transactions)
            .service(get_all_msg_ibc_transfer_transactions)
            .service(get_send_to_eth_transaction_totals)
    });

    let server = if SSL {
        let cert_chain = load_certs(&format!("/etc/letsencrypt/live/{}/fullchain.pem", DOMAIN));
        let keys = load_private_key(&format!("/etc/letsencrypt/live/{}/privkey.pem", DOMAIN));
        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, keys)
            .unwrap();

        info!("Binding to SSL");
        server.bind_rustls(format!("{}:{}", DOMAIN, PORT), config)?
    } else {
        server.bind(format!("{}:{}", DOMAIN, PORT))?
    };

    server.run().await?;

    Ok(())
}
