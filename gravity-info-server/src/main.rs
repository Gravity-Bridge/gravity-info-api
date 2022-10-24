#[macro_use]
extern crate lazy_static;

pub mod query_loop;
pub mod total_suppy;


use actix_web::{get, web, App, HttpServer, Responder, HttpResponse};
use query_loop::blockchain_info_thread;
use total_suppy::chain_total_supply_thread;
use crate::total_suppy::get_supply_info;


#[get("/total_supply")]
async fn get_total_supply() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => HttpResponse::Ok().json(v.total_liquid_supply),
        None => HttpResponse::InternalServerError().json("Info not yet generated, please query in 5 minutes")
    }
}

#[get("/supply_info")]
async fn get_all_supply_info() -> impl Responder {
    // if we have already computed supply info return it, if not return an error
    match get_supply_info() {
        Some(v) => HttpResponse::Ok().json(v),
        None => HttpResponse::InternalServerError().json("Info not yet generated, please query in 5 minutes")
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // starts background thread for gathering into
    blockchain_info_thread();
    // starts a background thread for generating the total supply numbers
    chain_total_supply_thread();
    HttpServer::new(|| {
        App::new()
            .route("/hello", web::get().to(|| async { "Hello World!" }))
            .service(greet)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
