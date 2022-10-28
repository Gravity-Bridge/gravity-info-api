export type BatchFees = {
    token: string,
    total_fees: number,
    tx_count: number
}

export type Attestation = {
    height: number,
    observed: boolean,
    votes: number
}

export type GravityParams = {
    bridge_ethereum_address: string,
    average_block_time: number,
    avearge_ethereum_block_time: number,
    target_batch_timeout: number,
    bridge_active: boolean,
    ethereum_blacklist: Array<string>,
    gravity_id: string,
    bridge_chain_id: number,
    signed_valsets_window: number,
    signed_batches_window: number,
    signed_logic_calls_window: number,
    unbond_slashing_valsets_window: number,
    valset_reward: Coin | null
}

export type Coin = {
    denom: string,
    amount: number,
}

export type Erc20Token = {
    amount: number,
    contract: string
}

export type TransactionBatch = {
    nonce: number,
    batch_timeout: number,
    transactions: Array<BatchTransaction>,
    total_fee: Erc20Token
    token_contract: string
}

export type BatchTransaction = {
    id: number,
    sender: string,
    destination: string,
    erc20_token: Erc20Token,
    erc20_fee: Erc20Token
}

export type GravityInfo = {
    pending_tx: Array<BatchFees>,
    pending_batches: Array<TransactionBatch>
    attestations: Array<Attestation>,
    params: GravityParams
}

export type ChainTotalSupplyNumbers = {
    total_liquid_supply: number,
    total_liquid_balances: number,
    total_unclaimed_rewards: number,
    total_nonvesting_staked: number,
    total_vesting: number,
    total_vesting_staked: number,
    total_vested: number,
}

export type SendToCosmosEvent = {
    erc20: string,
    sender: string,
    destination: string,
    validated_destination: string | null
    amount: number,
    event_nonce: number,
    block_height: number
}

export type TransactionBatchExecutedEvent = {
    batch_nonce: number,
    block_height: number,
    erc20: string,
    event_nonce: number,
}

export type ValsetUpdatedEvent = {
    valset_nonce: number,
    event_nonce: number,
    block_height: number,
    reward_amount: number,
    reward_token: string | null,
    members: Array<ValsetMember>

}

export type ValsetMember = {
    power: number,
    eth_address: string
}

export type Erc20DeployedEvent = {
    cosmos_denom: string,
    erc20_address: string,
    name: string,
    symbol: string,
    decimals: number,
    event_nonce: number,
    block_height: number,
}

export type LogicCallExecutedEvent = {
    invalidation_id: Array<number>,
    invalidation_nonce: number,
    return_data: Array<number>,
    event_nonce: number,
    block_height: number,
}

export type EthInfo = {
    deposit_events: Array<SendToCosmosEvent>,
    batch_events: Array<TransactionBatchExecutedEvent>,
    valset_updates: Array<ValsetUpdatedEvent>,
    erc20_deploys: Array<Erc20DeployedEvent>,
    logic_calls: Array<LogicCallExecutedEvent>,
}

export type Erc20Metadata = {
    address: string,
    decimals: number,
    symbol: string,
    exchange_rate: number | null
}
