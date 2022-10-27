#[macro_use]
extern crate lazy_static;

pub mod gravity_info;
pub mod tls;
pub mod total_suppy;

const DEVELOPMENT: bool = cfg!(feature = "development");
const SSL: bool = !DEVELOPMENT;
const DOMAIN: &str = if cfg!(test) || DEVELOPMENT {
    "localhost"
} else {
    "info.gravitychain.io"
};
const PORT: u16 = 9000;

use crate::gravity_info::get_erc20_metadata;
use crate::{tls::*, gravity_info::get_gravity_info};
use crate::total_suppy::get_supply_info;
use actix_cors::Cors;
use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use env_logger::Env;
use gravity_info::{blockchain_info_thread, get_eth_info};
use log::info;
use rustls::ServerConfig;
use total_suppy::chain_total_supply_thread;

#[get("/total_supply")]
async fn get_total_supply() -> impl Responder {
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    openssl_probe::init_ssl_cert_env_vars();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    // starts background thread for gathering into
    blockchain_info_thread();
    // starts a background thread for generating the total supply numbers
    chain_total_supply_thread();

    let server = HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_header()
                    .allow_any_method(),
            )
            .service(get_total_supply)
            .service(get_all_supply_info)
            .service(get_eth_bridge_info)
            .service(get_gravity_bridge_info)
            .service(erc20_metadata)
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
