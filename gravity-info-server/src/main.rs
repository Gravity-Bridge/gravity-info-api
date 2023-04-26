#[macro_use]
extern crate lazy_static;

pub mod gravity_info;
pub mod tls;
pub mod total_suppy;
pub mod volume;
pub mod transactions;

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
use log::info;
use rustls::ServerConfig;
use total_suppy::chain_total_supply_thread;
use volume::bridge_volume_thread;
use transactions::{transactions, ApiResponse, CustomMsgSendToEth};
use std::sync::Arc;
use rocksdb::DB;
use rocksdb::Options;
use tokio::runtime::Runtime;


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
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/transactions")]
async fn get_all_msg_send_to_eth_transactions(db: web::Data<Arc<DB>>) -> impl Responder {
    let mut response_data = Vec::new();

    let iterator = db.iterator(rocksdb::IteratorMode::Start);

    for item in iterator {
        match item {
            Ok((key, value)) => {
                let key_str = String::from_utf8_lossy(&key);
                if key_str.starts_with("msgSendToEth_") {
                    let msg_send_to_eth: CustomMsgSendToEth = serde_json::from_slice(&value).unwrap();
                    println!("Queried MsgSendToEth: {:?}", msg_send_to_eth);
                    response_data.push(ApiResponse {
                        tx_hash: key_str.replace("msgSendToEth_", ""),
                        data: serde_json::to_value(&msg_send_to_eth).unwrap(),
                    });
                }
            }
            Err(err) => {
                eprintln!("RocksDB iterator error: {}", err);
            }
        }
    }
    response_data.sort_by(|a, b| a.tx_hash.cmp(&b.tx_hash));
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
     transactions(api_db.clone(), db.clone(), &db_options);
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