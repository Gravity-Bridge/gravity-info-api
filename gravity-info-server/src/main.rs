#[macro_use]
extern crate lazy_static;

pub mod batch_relaying;
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

use crate::batch_relaying::generate_raw_batch_tx;
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

use std::sync::Arc;
use total_suppy::chain_total_supply_thread;
use transactions::database::transaction_info_thread;
use volume::bridge_volume_thread;

/// This is a helper api endpoint which generates an unsigned tx for a transaction batch sent from a given address
/// and returns it to the caller.
#[get("/batch_tx/{batch_nonce}")]
async fn generate_batch_tx(data: web::Path<(u64,)>) -> impl Responder {
    let nonce = data.into_inner().0;
    generate_raw_batch_tx(nonce).await
}

#[get("/total_supply")]
async fn get_total_supply() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => HttpResponse::Ok().json(v.total_supply),
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

/// If the liquid supply is lower than this value it's stale or otherwise invalid and we should
/// return an error.
pub const SUPPLY_CHECKPOINT: u128 = 500000000000000;
#[get("/total_liquid_supply")]
async fn get_total_liquid_supply() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => {
            if v.total_liquid_supply > SUPPLY_CHECKPOINT.into() {
                HttpResponse::Ok().json(v.total_liquid_supply)
            } else {
                error!("Invalid supply data, got total liquid supply of {:#?}", v);
                HttpResponse::InternalServerError()
                    .json("Invalid supply data, Gravity fullnode is stale")
            }
        }
        None => HttpResponse::InternalServerError()
            .json("Info not yet generated, please query in 5 minutes"),
    }
}

#[get("/supply_info")]
async fn get_all_supply_info() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => {
            if v.total_liquid_supply > SUPPLY_CHECKPOINT.into() {
                HttpResponse::Ok().json(v)
            } else {
                error!("Invalid supply data, got total liquid supply of {:#?}", v);
                HttpResponse::InternalServerError()
                    .json("Invalid supply data, Gravity fullnode is stale")
            }
        }
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
    transactions::endpoints::get_all_msg_send_to_eth_transactions(db).await
}

#[get("/transactions/ibc_transfer")]
async fn get_all_msg_ibc_transfer_transactions(db: web::Data<Arc<DB>>) -> impl Responder {
    transactions::endpoints::get_all_msg_ibc_transfer_transactions(db).await
}

#[get("/transactions/send_to_eth/time")]
async fn get_send_to_eth_transaction_totals(db: web::Data<Arc<DB>>) -> impl Responder {
    transactions::endpoints::get_send_to_eth_transaction_totals(db).await
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
            .service(generate_batch_tx)
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
