//! Yes computing the total supply of tokens on the chain does in fact require all the junk in this file.
//! This is mostly complexity around vesting and the fact that there's no convient function to total everything up
//! on the server side. Logic would dictate that you make the endpoint on client but this code uses highly parallel rust futures
//! to effectively query all the data in a reasonable amount of time and compute the result locally
//! This code provides a generic way to compute the total liquid supply for a cosmos chain across all vesting types

use crate::gravity_info::{GRAVITY_NODE_GRPC, GRAVITY_PREFIX, REQUEST_TIMEOUT};
use actix_web::rt::System;
use deep_space::client::types::AccountType;
use deep_space::client::PAGE;
use deep_space::error::CosmosGrpcError;
use deep_space::{Coin, Contact};
use futures::future::{join3, join_all};
use gravity_proto::cosmos_sdk_proto::cosmos::bank::v1beta1::query_client::QueryClient as BankQueryClient;
use gravity_proto::cosmos_sdk_proto::cosmos::bank::v1beta1::QueryBalanceRequest;
use gravity_proto::cosmos_sdk_proto::cosmos::distribution::v1beta1::query_client::QueryClient as DistQueryClient;
use gravity_proto::cosmos_sdk_proto::cosmos::distribution::v1beta1::QueryDelegationTotalRewardsRequest;
use gravity_proto::cosmos_sdk_proto::cosmos::staking::v1beta1::query_client::QueryClient as StakingQueryClient;
use gravity_proto::cosmos_sdk_proto::cosmos::staking::v1beta1::QueryDelegatorDelegationsRequest;
use gravity_proto::cosmos_sdk_proto::cosmos::vesting::v1beta1::BaseVestingAccount;
use log::{error, info, trace};
use num256::Uint256;
use serde::Serialize;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tonic::transport::channel::Channel;

// update once a day
const LOOP_TIME: Duration = Duration::from_secs(86400);
pub const GRAVITY_DENOM: &str = "ugraviton";

#[derive(Debug, Clone, Serialize)]
pub struct ChainTotalSupplyNumbers {
    /// All tokens that are 'liquid' meaning in a balance, claimable now as rewards
    /// or staked and eligeable to withdraw and spend, essentially just exludes vesting
    pub total_liquid_supply: Uint256,
    /// All tokens that are in users balances and can be sent instantly
    pub total_liquid_balances: Uint256,
    /// All tokens that are unclaimed as rewards
    pub total_unclaimed_rewards: Uint256,
    /// All tokens staked, but not vesting
    pub total_nonvesting_staked: Uint256,
    /// All tokens not yet vested, including those staked
    pub total_vesting: Uint256,
    /// All tokens that are vesting and staked
    pub total_vesting_staked: Uint256,
    /// All tokens that have vested so far
    pub total_vested: Uint256,
}

lazy_static! {
    static ref TOTAL_SUPPLY: Arc<RwLock<Option<ChainTotalSupplyNumbers>>> =
        Arc::new(RwLock::new(None));
}

fn set_supply_info(input: ChainTotalSupplyNumbers) {
    let mut r = TOTAL_SUPPLY.write().unwrap();
    *r = Some(input);
}

pub fn get_supply_info() -> Option<ChainTotalSupplyNumbers> {
    TOTAL_SUPPLY.read().unwrap().clone()
}

pub fn chain_total_supply_thread() {
    info!("Starting supply calculation thread");

    thread::spawn(move || loop {
        let runner = System::new();
        runner.block_on(async move {
            let contact = Contact::new(GRAVITY_NODE_GRPC, REQUEST_TIMEOUT, GRAVITY_PREFIX).unwrap();
            match compute_liquid_supply(&contact, GRAVITY_DENOM.to_string()).await {
                Ok(v) => {
                    info!("Successfully updated supply info!");
                    set_supply_info(v);
                }
                Err(e) => error!("Failed to update supply info with {:?}", e),
            }
        });
        thread::sleep(LOOP_TIME);
    });
}

/// This is extremely complicated with vesting, but people want to know
/// so we'll do an estimation, essentially what we need to do is iterate over
/// the entire set of accounts on chain and sum up tokens from non-module accounts
/// taking into account vesting by interpreting the vesting rules ourselves as we go
/// there is no neatly stored value of who has vested how much because doing so would be
/// impractical, if you have 100k vesting accounts that's 100k store changes per block to do
/// continuous vesting, it's intractable so instead liquid amounts are computed when a transfer
/// is attempted we're going to compute it all at once in this function. This function is useful
/// for any cosmos chain using standard vesting
///
/// Returns liquid supply (not including community pool, including staked but liquid tokens)
async fn compute_liquid_supply(
    contact: &Contact,
    denom: String,
) -> Result<ChainTotalSupplyNumbers, CosmosGrpcError> {
    let start = Instant::now();
    info!("Starting get all accounts");
    // start by getting every account on chain and every balance for every account
    let accounts = contact.get_all_accounts().await?;
    info!("Got all accounts after {}ms", start.elapsed().as_millis());
    let users = get_balances_for_accounts(accounts, denom.clone()).await?;
    info!(
        "Got all balances/vesting after {}ms",
        start.elapsed().as_millis()
    );
    // now that we have every account with every balance we can start computing the totals

    // all the tokens that are 'liquid' including staking rewards and non-vesting staking tokens
    let mut total_liquid_supply: Uint256 = 0u8.into();

    let mut total_liquid_balances: Uint256 = 0u8.into();
    let mut total_unclaimed_rewards: Uint256 = 0u8.into();
    let mut total_nonvesting_staked: Uint256 = 0u8.into();

    let mut total_vesting: Uint256 = 0u8.into();
    let mut total_vested: Uint256 = 0u8.into();
    let mut total_vesting_staked: Uint256 = 0u8.into();

    for user in users {
        match user.account {
            // account with no vesting, simple case, all is liquid
            AccountType::ProtoBaseAccount(_) => {
                total_liquid_balances += user.balance.clone();
                total_nonvesting_staked += user.total_staked.clone();
                total_unclaimed_rewards += user.unclaimed_rewards.clone();

                total_liquid_supply += user.balance;
                total_liquid_supply += user.unclaimed_rewards;
                total_liquid_supply += user.total_staked;
            }
            // account with periodic vesting, now we need to determine how much has vested then compare
            // that to their account balance
            AccountType::PeriodicVestingAccount(account_info) => {
                let vesting_start_time =
                    UNIX_EPOCH + Duration::from_secs(account_info.start_time as u64);
                let base = account_info.base_vesting_account.unwrap();
                let (total_delegated_free, total_delegated_vesting, original_vesting_amount) =
                    sum_vesting(base, denom.clone());
                // obvious stuff requiring no computation
                total_liquid_supply += user.unclaimed_rewards;
                total_liquid_supply += total_delegated_free.clone();
                total_vesting_staked += total_delegated_vesting.clone();
                total_nonvesting_staked += total_delegated_free;

                // vesting has started
                if vesting_start_time < SystemTime::now() {
                    let mut total_amount_vested: Uint256 = 0u8.into();
                    // seconds offset from vesting start time
                    let mut time_counter = 0;
                    for vesting_period in account_info.vesting_periods {
                        time_counter += vesting_period.length;
                        // if this vesting period has already elapsed, add the mount
                        if vesting_start_time + Duration::from_secs(time_counter as u64)
                            <= SystemTime::now()
                        {
                            // hack assumes vesting is only one coin
                            let amount: Coin = vesting_period.amount[0].clone().into();
                            assert_eq!(amount.denom, denom);
                            total_amount_vested += amount.amount;
                        }
                    }
                    let total_amount_still_vesting =
                        total_amount_vested.clone() - original_vesting_amount;

                    total_vested += total_amount_vested;
                    total_vesting += total_amount_still_vesting.clone();

                    let vesting_in_balance = total_amount_still_vesting - total_delegated_vesting;
                    // unvested tokens show up in the balance
                    // but unvested delegated tokens do not, in the case where a user
                    // has some vesting, some delegation, some balance, and some unclaimed rewards
                    total_liquid_supply += user.balance - vesting_in_balance;
                }
                // vesting has not started yet, in this case we subtract total vesting amount
                // from current balance, if the number is positive (staking could make it negative)
                // we add to our total
                else {
                    let vesting_in_balance =
                        original_vesting_amount.clone() - total_delegated_vesting;
                    total_vested += original_vesting_amount;
                    total_liquid_supply += user.balance - vesting_in_balance;
                }
            }
            AccountType::ContinuousVestingAccount(account_info) => {
                let vesting_start_time =
                    UNIX_EPOCH + Duration::from_secs(account_info.start_time as u64);
                let base = account_info.base_vesting_account.unwrap();
                let vesting_duration =
                    Duration::from_secs(base.end_time as u64 - account_info.start_time as u64);
                let (total_delegated_free, total_delegated_vesting, original_vesting_amount) =
                    sum_vesting(base, denom.clone());

                // obvious stuff requiring no computation
                total_unclaimed_rewards += user.unclaimed_rewards.clone();
                total_liquid_supply += user.unclaimed_rewards;
                total_liquid_supply += total_delegated_free.clone();
                total_vesting_staked += total_delegated_vesting.clone();
                total_nonvesting_staked += total_delegated_free;

                // vesting has started, since this is continuous we'll do a rough protection
                // between the start and the end time, determine what percentage has elapsed
                // and grant that as liquid
                if vesting_start_time < SystemTime::now() {
                    let elapsed_since_vesting_started = vesting_start_time.elapsed().unwrap();
                    let vesting_percent_complete = elapsed_since_vesting_started.as_secs() as f64
                        / vesting_duration.as_secs() as f64;
                    let original_vesting_amount_float: f64 =
                        original_vesting_amount.to_string().parse().unwrap();
                    let total_amount_vested: f64 =
                        original_vesting_amount_float * vesting_percent_complete;
                    let total_amount_vested: Uint256 = (total_amount_vested.ceil() as u128).into();

                    let total_amount_still_vesting =
                        total_amount_vested.clone() - original_vesting_amount;

                    total_vested += total_amount_vested;
                    total_vesting += total_amount_still_vesting.clone();

                    let vesting_in_balance = total_amount_still_vesting - total_delegated_vesting;
                    // unvested tokens show up in the balance
                    // but unvested delegated tokens do not, in the case where a user
                    // has some vesting, some delegation, some balance, and some unclaimed rewards
                    total_liquid_supply += user.balance - vesting_in_balance;
                }
                // vesting has not started yet, in this case we subtract total vesting amount
                // from current balance, if the number is positive (staking could make it negative)
                // we add to our total
                else {
                    let vesting_in_balance =
                        original_vesting_amount.clone() - total_delegated_vesting;
                    total_liquid_balances += user.balance.clone() - vesting_in_balance.clone();
                    total_vesting += original_vesting_amount;

                    total_liquid_supply += user.balance - vesting_in_balance;
                }
            }
            AccountType::DelayedVestingAccount(_) => todo!(),
            // module accounts are not liquid supply
            AccountType::ModuleAccount(_) => {}
            // it's locked, not liquid
            AccountType::PermenantLockedAccount(_) => {}
        }
    }

    info!("Finishes totals after {}ms", start.elapsed().as_millis());
    Ok(ChainTotalSupplyNumbers {
        total_liquid_supply,
        total_liquid_balances,
        total_unclaimed_rewards,
        total_nonvesting_staked,
        total_vesting,
        total_vesting_staked,
        total_vested,
    })
}

/// Dispatching utility function for building an array of joinable futures containing sets of batch requests
async fn get_balances_for_accounts(
    input: Vec<AccountType>,
    denom: String,
) -> Result<Vec<UserInfo>, CosmosGrpcError> {
    // handed tuned parameter for the ideal number of queryes per BankQueryClient
    const BATCH_SIZE: usize = 500;
    info!(
        "Querying {} accounts in {} batches of {}",
        input.len(),
        input.len() / BATCH_SIZE,
        BATCH_SIZE
    );
    let mut index = 0;
    let mut futs = Vec::new();
    while index + BATCH_SIZE < input.len() - 1 {
        futs.push(batch_query_user_information(
            &input[index..index + BATCH_SIZE],
            denom.clone(),
        ));
        index += BATCH_SIZE;
    }
    futs.push(batch_query_user_information(&input[index..], denom.clone()));

    let executed_futures = join_all(futs).await;
    let mut balances = Vec::new();
    for b in executed_futures {
        balances.extend(b?);
    }
    Ok(balances)
}

/// Utility function for batching balance requests so that they occupy a single bankqueryclient which represents a connection
/// to the rpc server, opening connections is overhead intensive so we want to do a few thousand requests per client to really
/// make it worth our while
async fn batch_query_user_information(
    input: &[AccountType],
    denom: String,
) -> Result<Vec<UserInfo>, CosmosGrpcError> {
    trace!("Starting batch of {}", input.len());
    let mut bankrpc = BankQueryClient::connect(GRAVITY_NODE_GRPC)
        .await?
        .accept_gzip();
    let mut distrpc = DistQueryClient::connect(GRAVITY_NODE_GRPC)
        .await?
        .accept_gzip();
    let mut stakingrpc = StakingQueryClient::connect(GRAVITY_NODE_GRPC)
        .await?
        .accept_gzip();

    let mut ret = Vec::new();
    for account in input {
        let res = merge_user_information(
            account.clone(),
            denom.clone(),
            &mut bankrpc,
            &mut distrpc,
            &mut stakingrpc,
        )
        .await?;
        ret.push(res);
    }
    trace!("Finished batch of {}", input.len());
    Ok(ret)
}

/// utility function for keeping the Account and Balance info
/// in the same scope rather than zipping them on return
async fn merge_user_information(
    account: AccountType,
    denom: String,
    bankrpc: &mut BankQueryClient<Channel>,
    distrpc: &mut DistQueryClient<Channel>,
    stakingrpc: &mut StakingQueryClient<Channel>,
) -> Result<UserInfo, CosmosGrpcError> {
    // required because dec coins are multiplied by 1*10^18
    const ONE_ETH: u128 = 10u128.pow(18);

    let address = account.get_base_account().address;
    let balance_fut = bankrpc.balance(QueryBalanceRequest {
        address: address.to_string(),
        denom: denom.clone(),
    });
    let delegation_rewards_fut =
        distrpc.delegation_total_rewards(QueryDelegationTotalRewardsRequest {
            delegator_address: address.to_string(),
        });
    let total_delegated_fut = stakingrpc.delegator_delegations(QueryDelegatorDelegationsRequest {
        delegator_addr: address.to_string(),
        pagination: PAGE,
    });

    let (balance, delegation_rewards, total_delegated) =
        join3(balance_fut, delegation_rewards_fut, total_delegated_fut).await;

    let balance = balance?.into_inner();
    let delegation_rewards = delegation_rewards?.into_inner();
    let delegated = total_delegated?.into_inner();

    let balance = match balance.balance {
        Some(v) => {
            let v: Coin = v.into();
            v.amount
        }
        None => 0u8.into(),
    };

    let mut delegation_rewards_total: Uint256 = 0u8.into();
    for reward in delegation_rewards.total {
        if reward.denom == denom {
            delegation_rewards_total += reward.amount.parse().unwrap();
        }
        // you can total non-native token rewards in an else case here
    }
    delegation_rewards_total /= ONE_ETH.into();

    let mut total_delegated: Uint256 = 0u8.into();
    for delegated in delegated.delegation_responses {
        if let Some(b) = delegated.balance {
            let b: Coin = b.into();
            assert_eq!(b.denom, denom);
            total_delegated += b.amount
        }
    }

    Ok(UserInfo {
        account,
        balance,
        unclaimed_rewards: delegation_rewards_total,
        total_staked: total_delegated,
    })
}

fn sum_vesting(input: BaseVestingAccount, denom: String) -> (Uint256, Uint256, Uint256) {
    let mut total_free = 0u8.into();
    let mut total_vesting = 0u8.into();
    let mut original_amount = 0u8.into();

    for coin in input.delegated_free {
        let coin: Coin = coin.into();
        assert_eq!(coin.denom, denom);
        total_free += coin.amount;
    }
    for coin in input.delegated_vesting {
        let coin: Coin = coin.into();
        assert_eq!(coin.denom, denom);
        total_vesting += coin.amount;
    }
    for coin in input.original_vesting {
        let coin: Coin = coin.into();
        assert_eq!(coin.denom, denom);
        original_amount += coin.amount;
    }

    (total_free, total_vesting, original_amount)
}

struct UserInfo {
    account: AccountType,
    balance: Uint256,
    unclaimed_rewards: Uint256,
    total_staked: Uint256,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_vesting_query() {
        env_logger::init();
        let contact = Contact::new(GRAVITY_NODE_GRPC, REQUEST_TIMEOUT, GRAVITY_PREFIX).unwrap();
        let supply = compute_liquid_supply(&contact, GRAVITY_DENOM.to_string())
            .await
            .unwrap();
        info!("Got a liquid supply of {:?}", supply);
    }
}
