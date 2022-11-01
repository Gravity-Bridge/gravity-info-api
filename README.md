# Gravity Info Dashboard

This repo contains frontend and backend code for deploying a Gravity information server, this server queries public full nodes and processes data about the chain in order to present the page https://info.gravitychain.io in addition to this page public API endpoints are provided

This server computes and displays info that would be otherwise difficult to access or compute. Such as daily and weekly bridge volume, vesting info, and parsed out transfer destinations.

Issues and pull requests for new endpoints or information formats are welcome.

## API Docs

### /bridge_volume

Provides weekly and daily volume information for [Gravity Bridge](https://etherscan.io/address/0xa4108aA1Ec4967F8b52220a4f7e94A8201F2D906#tokentxns). The value of all bridged tokens is converted to USDC and summed. Units here are in whole USDC. This endpoint is updated once a day.

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
  "weekly_outflow": 7078251.297628619
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
    }
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
      "erc20": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      "sender": "0xD544cF7Ca732D8e71775eA12a076A9a9C45ba951",
      "destination": "osmo1sp9lhefgwkq43ae5uzhkzydav7w9rtwy6zq50l",
      "validated_destination": "osmo1sp9lhefgwkq43ae5uzhkzydav7w9rtwy6zq50l",
      "amount": "100000000",
      "event_nonce": 19309,
      "block_height": "15868744"
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

Provides the total liquid supply of GRAV, or any Cosmos chain the server software is pointed at. Liquid supply excludes only module tokens and vesting tokens. Staked tokens and unclaimed rewards count in the total. Value return is ugravition and must be divided by `1*10^6` to display whole tokens. This value is updated once a day.

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

### /supply_info

Provides a breakdown of vesting versus non-vesting tokens for Gravity Bridge, value returned are in ugravition and must be divided by `1*10^6` to display whole tokens. This value is updated once a day.

- URL: `https://info.gravitychain.io:9000/supply_info`
- Method: `GET`
- URL Params: `None`
- Data Params: `None`
- Success Response:
  - Code: 200 OK
  - Contents:

```
{
  "total_liquid_supply": "423746179291553",
  "total_liquid_balances": "180142620528758",
  "total_unclaimed_rewards": "81132610438320",
  "total_nonvesting_staked": "162470948324475",
  "total_vesting": "1072829096565243",
  "total_vesting_staked": "896442811148458",
  "total_vested": "0"
}

```

- Error Response: `500 Server Error`

- Sample Call:

`curl https://info.gravitychain.io:9000/supply_info`

---