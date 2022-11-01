//! This file computes the total volume of Gravity bridge over daily, weekly, and monthly periods, this is an extremely time consuming task
//! becuase we iterate over the metadata of all erc20 in the bridge this task depends on the fast get info loop completing first

use actix_web::cookie::time::Instant;
use actix_web::rt::System;
use clarity::Uint256;
use futures::future::join;
use futures::future::join_all;
use gravity_utils::error::GravityError;
use log::{info, warn};
use serde::Serialize;
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};
use web30::client::Web3;
use web30::types::Log;

use crate::gravity_info::{get_erc20_metadata, get_gravity_info, Erc20Metadata, ETH_NODE_RPC};
use clarity::Address as EthAddress;

// update once a day
const LOOP_TIME: Duration = Duration::from_secs(86400);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const BLOCKS_PER_DAY: u128 = 7_200;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeVolumeNumbers {
    pub daily_volume: f64,
    pub daily_inflow: f64,
    pub daily_outflow: f64,
    pub weekly_volume: f64,
    pub weekly_inflow: f64,
    pub weekly_outflow: f64,
}

lazy_static! {
    static ref VOLUME: Arc<RwLock<Option<BridgeVolumeNumbers>>> = Arc::new(RwLock::new(None));
}

fn set_volume_info(input: BridgeVolumeNumbers) {
    let mut r = VOLUME.write().unwrap();
    *r = Some(input);
}

pub fn get_volume_info() -> Option<BridgeVolumeNumbers> {
    VOLUME.read().unwrap().clone()
}

pub fn bridge_volume_thread() {
    info!("Starting volume computation thread");

    thread::spawn(move || loop {
        let runner = System::new();
        runner.block_on(async move {
            let web3 = Web3::new(ETH_NODE_RPC, REQUEST_TIMEOUT);
            let metadata = get_erc20_metadata();
            let params = get_gravity_info();
            if let (Some(metadata), Some(params)) = (metadata, params) {
                let gravity_contract_address = params.params.bridge_ethereum_address;
                let latest_block = match web3.eth_block_number().await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Failed to get ETH block number with {:?}", e);
                        return;
                    }
                };
                let starting_block_daily = latest_block.clone() - BLOCKS_PER_DAY.into();
                let starting_block_weekly = latest_block.clone() - (BLOCKS_PER_DAY * 7).into();
                let daily_volume = get_bridge_volume_for_range(
                    starting_block_daily,
                    latest_block.clone(),
                    &metadata,
                    gravity_contract_address,
                    &web3,
                );
                let weekly_volume = get_bridge_volume_for_range(
                    starting_block_weekly,
                    latest_block,
                    &metadata,
                    gravity_contract_address,
                    &web3,
                );
                info!("Starting volume query");
                let start = Instant::now();
                let (daily_volume, weekly_volume) = join(daily_volume, weekly_volume).await;
                match (daily_volume, weekly_volume) {
                    (Ok(daily), Ok(weekly)) => {
                        set_volume_info(BridgeVolumeNumbers {
                            daily_volume: daily.volume,
                            daily_inflow: daily.inflow,
                            daily_outflow: daily.outflow,
                            weekly_volume: weekly.volume,
                            weekly_inflow: weekly.inflow,
                            weekly_outflow: weekly.outflow,
                        });
                        info!(
                            "Successfuly updated volume info in {}s!",
                            start.elapsed().as_seconds_f32()
                        );
                    }
                    (Err(e), _) => warn!("Could not get daily volume {:?}", e),
                    (_, Err(e)) => warn!("Could not get weekly volume {:?}", e),
                }
            }
        });
        if get_volume_info().is_some() {
            thread::sleep(LOOP_TIME);
        } else {
            // we haven't gotten any info yet, try again soon
            thread::sleep(Duration::from_secs(5));
        }
    });
}

#[derive(Debug, Clone, Serialize, Default)]
struct BridgeVolume {
    volume: f64,
    inflow: f64,
    outflow: f64,
}

/// Gets the bridge volume across all tokens for a provided block range
async fn get_bridge_volume_for_range(
    starting_block: Uint256,
    ending_block: Uint256,
    metadata: &[Erc20Metadata],
    gravity_contract_address: EthAddress,
    web3: &Web3,
) -> Result<BridgeVolume, GravityError> {
    // total volume in usdc
    let mut volume = 0u8.into();
    let mut inflow = 0u8.into();
    let mut outflow = 0u8.into();
    let mut futs = Vec::new();
    for token in metadata {
        let vol = get_gravity_volume_for_token(
            starting_block.clone(),
            ending_block.clone(),
            &token,
            gravity_contract_address,
            &web3,
        );
        futs.push(vol);
    }
    let futs = join_all(futs).await;
    for f in futs {
        let f = f?;
        volume += f.volume;
        inflow += f.inflow;
        outflow += f.outflow;
    }
    Ok(BridgeVolume {
        volume,
        inflow,
        outflow,
    })
}

/// Gets the volume of the Gravity contract over the provided
/// number of blocks for a given erc20
async fn get_gravity_volume_for_token(
    starting_block: Uint256,
    ending_block: Uint256,
    erc20: &Erc20Metadata,
    gravity_contract_address: EthAddress,
    web3: &Web3,
) -> Result<BridgeVolume, GravityError> {
    if let Some(exchange_rate) = erc20.exchange_rate.clone() {
        let mut volume: f64 = 0u8.into();
        let mut inflow: f64 = 0u8.into();
        let mut outflow: f64 = 0u8.into();

        let decimals: u32 = erc20.decimals.to_string().parse().unwrap();
        let exchange_rate: f64 = exchange_rate.to_string().parse().unwrap();
        info!("Searching events for {}", erc20.symbol);
        // tiny block range becuase of the huge amount of events
        // these contracts prodcue
        let blocks_to_search: Uint256 = 500u16.into();
        let mut current_block = starting_block;
        while current_block.clone() + blocks_to_search.clone() < ending_block {
            let logs = web3
                .check_for_events(
                    current_block.clone(),
                    Some(current_block.clone() + blocks_to_search.clone()),
                    vec![erc20.address],
                    vec!["Transfer(address,address,uint256)"],
                )
                .await?;

            let (v, i, o) = sum_logs(logs, gravity_contract_address, decimals, exchange_rate)?;
            volume += v;
            inflow += i;
            outflow += o;

            current_block = current_block + blocks_to_search.clone();
        }
        let logs = web3
            .check_for_events(
                current_block.clone(),
                Some(ending_block),
                vec![erc20.address],
                vec!["Transfer(address,address,uint256)"],
            )
            .await?;
        let (v, i, o) = sum_logs(logs, gravity_contract_address, decimals, exchange_rate)?;
        volume += v;
        inflow += i;
        outflow += o;

        Ok(BridgeVolume {
            volume,
            inflow,
            outflow,
        })
    } else {
        // no exchange rate, ignore
        Ok(BridgeVolume::default())
    }
}

fn sum_logs(
    logs: Vec<Log>,
    gravity_contract_address: EthAddress,
    decimals: u32,
    exchange_rate: f64,
) -> Result<(f64, f64, f64), GravityError> {
    let mut volume: f64 = 0u8.into();
    let mut inflow: f64 = 0u8.into();
    let mut outflow: f64 = 0u8.into();
    for l in logs {
        let from = EthAddress::from_slice(&l.topics[1][12..32])?;
        let to = EthAddress::from_slice(&l.topics[2][12..32])?;
        let amount = Uint256::from_bytes_be(&l.data[0..32]);
        // unit conversion to get to whole dollars float caveats about
        // rounding errors apply
        let amount: f64 = amount.to_string().parse().unwrap();
        let amount = amount / 10u128.pow(decimals) as f64;
        let amount = amount * exchange_rate;
        let amount = amount / 10u128.pow(6) as f64;
        if to == gravity_contract_address {
            volume += amount;
            inflow += amount
        } else if from == gravity_contract_address {
            volume += amount;
            outflow += amount;
        }
    }
    Ok((volume, inflow, outflow))
}
