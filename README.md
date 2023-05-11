# Gravity Info Dashboard

This repo contains frontend and backend code for deploying a Gravity information server, this server queries public full nodes and processes data about the chain in order to present the page https://info.gravitychain.io in addition to this page public API endpoints are provided

This server computes and displays info that would be otherwise difficult to access or compute. Such as daily and weekly bridge volume, vesting info, and parsed out transfer destinations.

Issues and pull requests for new endpoints or information formats are welcome.

## API Docs

### /bridge_volume

Provides monthly, weekly, and daily volume information for [Gravity Bridge](https://etherscan.io/address/0xa4108aA1Ec4967F8b52220a4f7e94A8201F2D906#tokentxns). The value of all bridged tokens is converted to USDC and summed. Units here are in whole USDC. This endpoint is updated once a day.

- URL: `https://info.gravitychain.io:9000/bridge_volume`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
{
  "daily_volume": 21220096.012809116,
  "daily_inflow": 20476817.14186712,
  "daily_outflow": 743278.8709419968,
  "weekly_volume": 45889152.25069955,
  "weekly_inflow": 38810900.95307093,
  "weekly_outflow": 7078251.297628619,
  "monthly_volume": 48873709.034694746,
  "monthly_inflow": 21492436.59797602,
  "monthly_outflow": 27381272.43671871
}
```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/bridge_volume`

---

### /erc20_metadata

Provides a list of every ERC20 that is in the [Gravity Bridge solidity contract](https://etherscan.io/address/0xa4108aA1Ec4967F8b52220a4f7e94A8201F2D906#tokentxns) along with symbol + decimals metadata. If this ERC20 has a Uniswap v2 or v3 pair an exchange rate is provided. The exchange rate is the amount of USDC 1 unit of the input token would buy. For example UNI below has a value of `"exchange_rate": "7259165"` meaning 1 UNI is worth `7259165` base units of USDC. Since USDC is a 6 decimal token we divide by `1*10^6` and get $7.25. This endpoint is updated every 30 seconds and does not rate limit queries.

- URL: `https://info.gravitychain.io:9000/erc20_metadata`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
[
  {
    "address": "0x147faF8De9d8D8DAAE129B187F0D02D819126750",
    "decimals": "18",
    "symbol": "GEO",
    "exchange_rate": null
  },
  {
    "address": "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984",
    "decimals": "18",
    "symbol": "UNI",
    "exchange_rate": "7259165"
  },
]


```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/erc20_metadata`

---

### /gravity_bridge_info

Provides info about the state of the Gravity Bridge module. Including the batch queue going out to Ethereum and the oracle bringing events in from Ethereum. This is updated every 30 seconds and ther is no rate limit on querying.

- URL: `https://info.gravitychain.io:9000/gravity_bridge_info`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
{
     "pending_tx": [
    {
      "token": "0xe9B076B476D8865cDF79D1Cf7DF420EE397a7f75",
      "total_fees": "10000000000",
      "tx_count": 1
    }
  ],
  "pending_batches": [
    {
      "nonce": 6388,
      "batch_timeout": 15876299,
      "transactions": [
        {
          "id": 7153,
          "sender": "gravity1eppzhlpczlap8he2h95yedzrwm96496c0hup0g",
          "destination": "0xC7172CeD2d1BE642bad2dC051A208F6cFC89aEa0",
          "erc20_token": {
            "amount": "6000000",
            "contract": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
          },
          "erc20_fee": {
            "amount": "6385782",
            "contract": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
          }
        }
      ],
      "total_fee": {
        "amount": "16406414",
        "contract": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
      },
      "token_contract": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    }
  ],
  "attestations": [
    {
      "height": 4303597,
      "observed": true,
      "votes": 146
    }
  ],
  "params": {
    "bridge_ethereum_address": "0xa4108aA1Ec4967F8b52220a4f7e94A8201F2D906",
    "average_block_time": 6282,
    "average_ethereum_block_time": 12020,
    "target_batch_timeout": 7200000,
    "bridge_active": true,
    "ethereum_blacklist": [
      "0x8576aCC5C05D6Ce88f4e49bf65BdF0C62F91353C",
    ],
    "gravity_id": "gravity-bridge-mainnet",
    "bridge_chain_id": 1,
    "signed_valsets_window": 10000,
    "signed_batches_window": 10000,
    "signed_logic_calls_window": 10000,
    "unbond_slashing_valsets_window": 10000,
    "valset_reward": {
      "amount": "0",
      "denom": ""
    },
    "min_chain_fee_basis_points": 2
  }
 
}
```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/gravity_bridge_info`

---


### /eth_bridge_info

Provides parsed Ethereum events from the [Gravity Bridge solidity contract](https://etherscan.io/address/0xa4108aA1Ec4967F8b52220a4f7e94A8201F2D906#tokentxns). This events list is updated every 30 seconds and there is no rate limit on querying.

- URL: `https://info.gravitychain.io:9000/eth_bridge_info`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
{
  "deposit_events": [
    {
      "erc20": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      "sender": "0xb5e452a90280A978aA8DAe4306F960167c7C528A",
      "destination": "canto1khj992gzsz5h325d4epsd7tqze78c552gc870p",
      "amount": "200000000000000000",
      "event_nonce": 19443,
      "block_height": "15876508",
      "confirmed": true,
      "blocks_until_confirmed": "0",
      "seconds_until_confirmed": "0"
    },
    {
      "erc20": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      "sender": "0xFe9707E8d9B9436f99633987b400bbdf6a5Ab072",
      "destination": "canto1l6ts06xeh9pklxtr8xrmgq9mma494vrjrr6pjf",
      "amount": "340000000000000000",
      "event_nonce": 19444,
      "block_height": "15876578",
      "confirmed": false,
      "blocks_until_confirmed": "14",
      "seconds_until_confirmed": "168"
    },
  ],
  "batch_events": [
    {
      "batch_nonce": 6312,
      "block_height": "15869111",
      "erc20": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      "event_nonce": 19315
    }
  ],
  "valset_updates": [
     {
      "valset_nonce": 796,
      "event_nonce": 19428,
      "block_height": "15875929",
      "reward_amount": "0",
      "reward_token": null,
      "members": [
        {
          "power": 504447949,
          "eth_address": "0x30d19C8a86C07991328C83FDB571A0De90A7290c"
        }
      ]
    }
  ],
  "erc20_deploys": [
    {
      "cosmos_denom": "ibc/D157AD8A50DAB0FC4EB95BBE1D9407A590FA2CDEE04C90A76C005089BF76E519",
      "erc20_address": "0xe9B076B476D8865cDF79D1Cf7DF420EE397a7f75",
      "name": "Unification",
      "symbol": "FUND",
      "decimals": 9,
      "event_nonce": 19411,
      "block_height": "15875392"
    },
  ],
  "logic_calls": []

```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/eth_bridge_info`

---

### /total_supply

Provides the total supply of GRAV, or any Cosmos chain the server software is pointed at. This is inclusive of the community pool, vesting tokens, staked tokens, and unclaimed rewards. Value return is ugravition and must be divided by `1*10^6` to display whole tokens. This value is updated once a day.

- URL: `https://info.gravitychain.io:9000/total_supply`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
"423746179291553"
```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/total_supply`

---

### /total_liquid_supply

Provides the total liquid supply of GRAV, or any Cosmos chain the server software is pointed at. Liquid supply excludes only module tokens and vesting tokens. Staked tokens and unclaimed rewards count in the total. Value return is ugravition and must be divided by `1*10^6` to display whole tokens. This value is updated once a day.

- URL: `https://info.gravitychain.io:9000/total_liquid_supply`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
"423746179291553"
```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/total_liquid_supply`

---

### /supply_info

Provides a breakdown of vesting versus non-vesting tokens for Gravity Bridge, value returned are in ugravition and must be divided by `1*10^6` to display whole tokens. This value is updated once a day.

* total_supply: The total supply of tokens in existance.
* community_pool: The total amount of tokens in the community pool subject to use by governance vote
* total_liquid_supply: All tokens that are not vesting and not in the community pool, this includes staked tokens and unclaimed staking rewards.
* total_liquid_balances: Tokens that are avaialble to be sent immeidately, so tokens that are not staked and not vesting.
* total_nonvesting_staked: These tokens are liquid (eg not vesting) and currently staked.
* total_vesting: A sum of all tokens that are not yet vested but will become liquid at some point in the future.
* total_vesting_staked: All tokens that are vesting and also staked
* total_vested: The amount of tokens that where once vesting but are now liquid

- URL: `https://info.gravitychain.io:9000/supply_info`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
{
  "total_supply": "2489386289699730",
  "community_pool": "938460578037767",
  "total_liquid_supply": "475122384773913",
  "total_liquid_balances": "151777718973370",
  "total_unclaimed_rewards": "107181985809999",
  "total_nonvesting_staked": "192953527166768",
  "total_vesting": "1050344613544263",
  "total_vesting_staked": "897039356148458",
  "total_vested": "22484483020980"
}


```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/supply_info`

---

### /transactions

Provides Gravity Bridge transaction info. Currently two message types are supported **MsgSendToEth** & **MsgTransfer**.

**MsgSendToEth** is the message type used to bridge assets from the Cosmos side to Ethereum.
- URL: `https://info.gravitychain.io:9000/transactions/send_to_eth`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
  {
    "tx_hash": "9EA7C11DB18B87111E2679F3FFACC2B0C77135C60A05B7836F404B5F93EF7D18",
    "data": {
      "amount": [
        {
          "amount": "89200000000000000",
          "denom": "gravity0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        }
      ],
      "bridge_fee": [
        {
          "amount": "3200000000000000",
          "denom": "gravity0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        }
      ],
      "chain_fee": [],
      "eth_dest": "0xf0f08f640d5553e79b91296dba6c3f10521e5174",
      "sender": "gravity1xq7j6pr0zphuq6elxmrg98zkm57u36pvz2uwcc"
    }
  }
```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/transactions/send_to_eth`

**MsgTransfer** is the message type used to transfer assets in between IBC enabled Cosmos chains.

- URL: `https://info.gravitychain.io:9000/transactions/ibc_transfer`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
  {
    "tx_hash": "0000405E464C64DE8537B594742CBD9D7E0AD8EEFDB118158AC4582FFE101A10",
    "data": {
      "receiver": "persistence1ac05mw63eury6arcux7u2qtxwxq68qvefxqczm",
      "sender": "gravity1apkwuud8qdkw3nectycl7d46j5jvqs4kq8nhhf",
      "source_channel": "channel-24",
      "source_port": "transfer",
      "timeout_height": {
        "revision_height": 4954551,
        "revision_number": 1
      },
      "timeout_timestamp": 0,
      "token": [
        {
          "amount": "150000000000000000000",
          "denom": "gravity0xfB5c6815cA3AC72Ce9F5006869AE67f18bF77006"
        }
      ]
    }
  }
```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/transactions/ibc_transfer`

---