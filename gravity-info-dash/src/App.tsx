import React, { useEffect, useState } from 'react';
import './App.css';
import {
  Spinner,
  CardBody,
  CardTitle,
  Card,
  CardSubtitle,
  ButtonGroup,
  Button,
  Table
} from 'reactstrap';
import {
  Attestation,
  BatchFees,
  BatchTransaction,
  ChainTotalSupplyNumbers,
  Erc20Metadata,
  EthInfo,
  GravityInfo,
  DepositWithMetadata,
  TransactionBatch,
  VolumeInfo,
  EvmChainConfig
} from './types';

// 5 seconds
const UPDATE_TIME = 5000;
export const SERVER_URL = (
  process.env.REACT_APP_BACKEND || window.location.origin
).replace(/\/?$/, '/');

const callMethodFromUrl = async (url: string, callback: Function) => {
  const request_url = SERVER_URL + url;
  const requestOptions: any = {
    method: 'GET',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/json'
    }
  };
  try {
    const result = await fetch(request_url, requestOptions);
    const json = await result.json();
    callback(json);
  } catch (ex) {
    console.log(ex);
  }
};

function App() {
  document.title = 'Gravity Bridge Info';
  const [gravityBridgeInfo, setGravityBridgeInfo] =
    useState<GravityInfo | null>(null);
  const [ethBridgeInfo, setEthBridgeInfo] = useState<EthInfo | null>(null);
  const [supplyInfo, setSupplyInfo] = useState<ChainTotalSupplyNumbers | null>(
    null
  );
  const [erc20Metadata, setErc20Metadata] =
    useState<Array<Erc20Metadata> | null>(null);
  const [volumeInfo, setVolumeInfo] = useState<VolumeInfo | null>(null);

  const [evmChainConfigs, setEvmChainConfigs] = useState<Array<EvmChainConfig>>(
    []
  );

  const [evmChainPrefix, setEvmChainPrefix] = useState<string | null>(null);

  const getEvmChainConfigs = async () => {
    await callMethodFromUrl('evm_chain_configs', (json: EvmChainConfig[]) => {
      if (json.length) {
        setEvmChainPrefix(json[0].prefix);
        setEvmChainConfigs(json);
      }
    });
  };

  const getGravityInfo = async () => {
    await callMethodFromUrl(
      `gravity_bridge_info?evm_chain_prefix=${evmChainPrefix}`,
      setGravityBridgeInfo
    );
  };
  const getEthInfo = async () => {
    await callMethodFromUrl(
      `eth_bridge_info?evm_chain_prefix=${evmChainPrefix}`,
      (json: EthInfo) => {
        // reverse so these show up in reverse cronological order
        json.batch_events.reverse();
        json.deposit_events.reverse();
        json.logic_calls.reverse();
        json.valset_updates.reverse();
        setEthBridgeInfo(json);
      }
    );
  };
  const getDistributionInfo = async () => {
    await callMethodFromUrl('supply_info', setSupplyInfo);
  };
  const getErc20Metadata = async () => {
    await callMethodFromUrl(
      `erc20_metadata?evm_chain_prefix=${evmChainPrefix}`,
      setErc20Metadata
    );
  };
  const getVolumeInfo = async () => {
    await callMethodFromUrl(
      `bridge_volume?evm_chain_prefix=${evmChainPrefix}`,
      setVolumeInfo
    );
  };

  useEffect(() => {
    getEvmChainConfigs();
    //eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    let interval: any = null;
    // only running when there is evmChainPrefix
    if (evmChainPrefix) {
      const update = async () => {
        await Promise.all([
          getDistributionInfo(),
          getGravityInfo(),
          getEthInfo(),
          getErc20Metadata(),
          getVolumeInfo()
        ]);
        interval = setTimeout(update, UPDATE_TIME);
      };
      update();
    }
    return () => clearInterval(interval);
    //eslint-disable-next-line react-hooks/exhaustive-deps
  }, [evmChainPrefix]);

  if (
    gravityBridgeInfo == null ||
    typeof gravityBridgeInfo === 'string' ||
    ethBridgeInfo == null ||
    supplyInfo == null ||
    typeof supplyInfo === 'string' ||
    erc20Metadata == null ||
    volumeInfo == null ||
    typeof volumeInfo === 'string'
  ) {
    return (
      <div className="App-header" style={{ display: 'flex', flexWrap: 'wrap' }}>
        <Spinner color="danger" type="grow">
          Loading...
        </Spinner>
      </div>
    );
  }

  let bridge_address = gravityBridgeInfo.params.bridge_ethereum_address;
  let etherscanBase = 'https://etherscan.io/address/';
  let etherscanBlockBase = 'https://etherscan.io/block/';
  let etherscanLink = etherscanBase + bridge_address;

  return (
    <div className="App-header" style={{ display: 'flex', flexWrap: 'wrap' }}>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between'
        }}
      >
        <div style={{ display: 'flex', flexDirection: 'column' }}>
          <div style={{ padding: 5 }}>
            <Card
              className="ParametersCard"
              style={{ borderRadius: 8, padding: 20 }}
            >
              <CardBody>
                <CardTitle tag="h4">Evm chains</CardTitle>
                <ButtonGroup size="sm">
                  {evmChainConfigs.map((evmChainConfig) => (
                    <Button
                      outline
                      color="primary"
                      title={evmChainConfig.rpc}
                      active={evmChainConfig.prefix === evmChainPrefix}
                      key={evmChainConfig.prefix}
                      onClick={() => {
                        setEvmChainPrefix(evmChainConfig.prefix);
                      }}
                    >
                      {evmChainConfig.prefix}
                    </Button>
                  ))}
                </ButtonGroup>
              </CardBody>
            </Card>
          </div>
          <div style={{ padding: 5 }}>
            <Card
              className="ParametersCard"
              style={{ borderRadius: 8, padding: 20 }}
            >
              <CardBody>
                <CardTitle tag="h4">Incoming transactions</CardTitle>
                <Table style={{ borderSpacing: 10, fontSize: 15 }}>
                  <thead>
                    <tr>
                      <th>Token</th>
                      <th>Value</th>
                      <th>Source</th>
                      <th>Destination</th>
                      <th>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {ethBridgeInfo.deposit_events.map(
                      (sendToCosmos: DepositWithMetadata) => (
                        <tr key={sendToCosmos.event_nonce}>
                          <td>
                            {
                              getMetadataFromList(
                                sendToCosmos.erc20,
                                erc20Metadata
                              )?.symbol
                            }
                          </td>
                          <td>
                            {amountToFraction(
                              sendToCosmos.erc20,
                              sendToCosmos.amount,
                              erc20Metadata
                            )}
                          </td>
                          <td>
                            <a href={etherscanBase + sendToCosmos.sender}>
                              {sendToCosmos.sender}
                            </a>
                          </td>
                          <td>
                            <a
                              href={cosmosAddressToExplorerLink(
                                sendToCosmos.destination
                              )}
                            >
                              {sendToCosmos.destination}
                            </a>
                          </td>
                          <td>
                            {printTxStatus(
                              sendToCosmos,
                              gravityBridgeInfo.attestations
                            )}
                          </td>
                        </tr>
                      )
                    )}
                  </tbody>
                </Table>
              </CardBody>
            </Card>
          </div>
        </div>
        <div
          style={{
            display: 'flex',
            flexDirection: 'column'
          }}
        >
          <div style={{ padding: 5 }}>
            <Card
              className="ParametersCard"
              style={{ borderRadius: 8, padding: 20 }}
            >
              <CardBody>
                <CardTitle tag="h4">Transaction Queue</CardTitle>
                <CardSubtitle style={{ fontSize: 15 }}>
                  These transactions are not yet in batches, a batch will be
                  reqested when the fee amount exceeds the cost to execute on
                  Ethereum
                </CardSubtitle>
                <Table
                  borderless
                  size="sm"
                  responsive
                  style={{ borderSpacing: 10, fontSize: 15 }}
                >
                  <thead>
                    <tr>
                      <th>Token</th>
                      <th>Num Transactions</th>
                      <th>Total Fees</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gravityBridgeInfo.pending_tx.map(
                      (batchFees: BatchFees, ind) => (
                        <tr key={ind}>
                          <td>
                            {
                              getMetadataFromList(
                                batchFees.token,
                                erc20Metadata
                              )?.symbol
                            }
                          </td>
                          <td>{batchFees.tx_count}</td>
                          <td>
                            {amountToFraction(
                              batchFees.token,
                              batchFees.total_fees,
                              erc20Metadata
                            )}
                          </td>
                        </tr>
                      )
                    )}
                  </tbody>
                </Table>
              </CardBody>
            </Card>
          </div>
          <div style={{ padding: 5 }}>
            <Card
              className="ParametersCard"
              style={{ borderRadius: 8, padding: 20 }}
            >
              <CardBody>
                <CardTitle tag="h4">Batch Queue</CardTitle>
                <CardSubtitle style={{ fontSize: 15 }}>
                  These transactions are in batches and waiting to be relayed to
                  Ethereum
                </CardSubtitle>
                {getNotExecutedBatches(gravityBridgeInfo, ethBridgeInfo).map(
                  (batch: TransactionBatch) => (
                    <Card key={batch.nonce}>
                      <CardBody>
                        <CardTitle tag="h5">
                          {' '}
                          Batch #{batch.nonce}{' '}
                          {
                            getMetadataFromList(
                              batch.token_contract,
                              erc20Metadata
                            )?.symbol
                          }
                        </CardTitle>
                        <div style={{ fontSize: 15 }}>
                          Total Fees:{' '}
                          {amountToFraction(
                            batch.token_contract,
                            batch.total_fee.amount,
                            erc20Metadata
                          )}
                        </div>
                        <div style={{ fontSize: 15 }}>
                          Timeout:{' '}
                          <a href={etherscanBlockBase + batch.batch_timeout}>
                            {batch.batch_timeout}
                          </a>
                        </div>
                        <Table style={{ borderSpacing: 10, fontSize: 15 }}>
                          <thead>
                            <tr>
                              <th>To</th>
                              <th>From</th>
                              <th>Amount / Fee</th>
                            </tr>
                          </thead>
                          <tbody>
                            {batch.transactions.map(
                              (batchTx: BatchTransaction) => (
                                <tr key={batchTx.id}>
                                  <td>
                                    <a
                                      href={etherscanBase + batchTx.destination}
                                    >
                                      {batchTx.destination}
                                    </a>
                                  </td>
                                  <td>
                                    <a
                                      href={cosmosAddressToExplorerLink(
                                        batchTx.sender
                                      )}
                                    >
                                      {batchTx.sender}
                                    </a>
                                  </td>
                                  <td>
                                    {amountToFraction(
                                      batchTx.erc20_token.contract,
                                      batchTx.erc20_token.amount,
                                      erc20Metadata
                                    )}
                                    /
                                    {amountToFraction(
                                      batchTx.erc20_token.contract,
                                      batchTx.erc20_fee.amount,
                                      erc20Metadata
                                    )}
                                  </td>
                                </tr>
                              )
                            )}
                          </tbody>
                        </Table>
                      </CardBody>
                    </Card>
                  )
                )}
              </CardBody>
            </Card>
          </div>
          <div style={{ padding: 5 }}>
            <Card
              className="ParametersCard"
              style={{ borderRadius: 8, padding: 25 }}
            >
              <CardBody>
                <CardTitle tag="h4">Current Gravity Parameters</CardTitle>
                <div style={{ fontSize: 15 }}>
                  Ethereum Contract Address:{' '}
                  <a href={etherscanLink}>{bridge_address}</a>
                </div>
                <div style={{ fontSize: 15 }}>
                  Bridge Active:{' '}
                  {String(gravityBridgeInfo.params.bridge_active)}
                </div>
                <div style={{ fontSize: 15 }}>
                  Target Batch Timeout:{' '}
                  {gravityBridgeInfo.params.target_batch_timeout /
                    1000 /
                    (60 * 60)}{' '}
                  hours
                </div>
              </CardBody>
            </Card>
          </div>

          <div style={{ display: 'flex' }}>
            <div style={{ padding: 5, flex: 1 }}>
              <Card
                className="ParametersCard"
                style={{ borderRadius: 8, padding: 25 }}
              >
                <CardBody>
                  <CardTitle tag="h4">Gravity Supply Info</CardTitle>
                  <div style={{ fontSize: 15 }}>
                    Total Supply:{' '}
                    {(supplyInfo.total_supply / 10 ** 12).toFixed(2)}M Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Community Pool:{' '}
                    {(supplyInfo.community_pool / 10 ** 12).toFixed(2)}M
                    Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Liquid (Not Vesting):{' '}
                    {(supplyInfo.total_liquid_supply / 10 ** 12).toFixed(2)}M
                    Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Liquid (Not Vesting) and staked:{' '}
                    {(supplyInfo.total_nonvesting_staked / 10 ** 12).toFixed(2)}
                    M Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Unclaimed staking rewards:{' '}
                    {(supplyInfo.total_unclaimed_rewards / 10 ** 12).toFixed(2)}
                    M Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Unvested: {(supplyInfo.total_vesting / 10 ** 12).toFixed(2)}
                    M Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Unvested Staked:{' '}
                    {(supplyInfo.total_vesting_staked / 10 ** 12).toFixed(2)}M
                    Graviton
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Vested: {(supplyInfo.total_vested / 10 ** 12).toFixed(2)}M
                    Graviton
                  </div>
                </CardBody>
              </Card>
            </div>
            <div style={{ padding: 5, flex: 1 }}>
              <Card
                className="ParametersCard"
                style={{ borderRadius: 8, padding: 25 }}
              >
                <CardBody>
                  <CardTitle tag="h4">Gravity Volume</CardTitle>
                  <div style={{ fontSize: 15 }}>
                    Daily Volume $
                    {(volumeInfo.daily_volume / 10 ** 6).toFixed(2)}M
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Daily Inflow $
                    {(volumeInfo.daily_inflow / 10 ** 6).toFixed(2)}M
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Daily Outflow $
                    {(volumeInfo.daily_outflow / 10 ** 6).toFixed(2)}M
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Weekly Volume $
                    {(volumeInfo.weekly_volume / 10 ** 6).toFixed(2)}M
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Weekly Inflow $
                    {(volumeInfo.weekly_inflow / 10 ** 6).toFixed(2)}M
                  </div>
                  <div style={{ fontSize: 15 }}>
                    Weekly Outflow $
                    {(volumeInfo.weekly_outflow / 10 ** 6).toFixed(2)}M
                  </div>
                </CardBody>
              </Card>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/// Inefficient utility function to lookup token metadata, should be using a map
/// of some kind
function getMetadataFromList(erc20: string, metadata: Array<Erc20Metadata>) {
  var arrayLength = metadata.length;
  for (var i = 0; i < arrayLength; i++) {
    if (metadata[i].address === erc20) {
      return metadata[i];
    }
  }
  return null;
}

/// returns a readable fraction value for a given erc20 amount, if the exchange rate is populated
/// it is used to display token value / dollar value
function amountToFraction(
  erc20: string,
  amount: number,
  metadata: Array<Erc20Metadata>
) {
  let tokenInfo = getMetadataFromList(erc20, metadata);
  if (tokenInfo == null) {
    return 0;
  }
  let fraction = amount / 10 ** tokenInfo.decimals;
  if (tokenInfo.exchange_rate == null) {
    return fraction.toFixed(2);
  } else {
    let dollar_value = fraction * (tokenInfo.exchange_rate / 10 ** 6);
    return '$' + dollar_value.toFixed(2);
  }
}

/// Takes both info structs to cross compare and display batches that have not yet been
/// executed without waiting 20 minutes for Gravity to know that a batch has been executed
/// on Ethereum
function getNotExecutedBatches(
  gravityBridgeInfo: GravityInfo,
  ethBridgeInfo: EthInfo
) {
  let ret = [];
  var arrayLength = gravityBridgeInfo.pending_batches.length;
  for (var i = 0; i < arrayLength; i++) {
    if (
      !alreadyExecuted(
        gravityBridgeInfo.pending_batches[i].nonce,
        ethBridgeInfo
      )
    ) {
      ret.push(gravityBridgeInfo.pending_batches[i]);
    }
  }
  return ret;
}

/// Checks if a batch has already executed on Ethereum but GB does not
/// know it yet by searching the eth events history
function alreadyExecuted(batch_nonce: number, ethBridgeInfo: EthInfo) {
  var arrayLength = ethBridgeInfo.batch_events.length;
  for (var i = 0; i < arrayLength; i++) {
    if (ethBridgeInfo.batch_events[i].batch_nonce === batch_nonce) {
      return true;
    }
  }
  return false;
}

/// Takes various cosmos addresses to create a proper mintscan link
function cosmosAddressToExplorerLink(input: string) {
  let gravBase = 'https://mintscan.io/gravity-bridge/account/';
  let osmoBase = 'https://mintscan.io/osmosis/account/';
  let crescentBase = 'https://mintscan.io/crescent/account/';
  let cantoBase = 'https://explorer.nodestake.top/canto/account/';
  let mantleBase = 'https://mintscan.io/mantle/account/';
  if (input.startsWith('gravity')) {
    return gravBase + input;
  } else if (input.startsWith('canto')) {
    return cantoBase + input;
  } else if (input.startsWith('osmosis')) {
    return osmoBase + input;
  } else if (input.startsWith('cre')) {
    return crescentBase + input;
  } else if (input.startsWith('mantle')) {
    return mantleBase + input;
  } else {
    return input;
  }
}

// takes a send to Cosmos event and determines its status
function printTxStatus(event: DepositWithMetadata, events: Array<Attestation>) {
  if (event.confirmed) {
    return 'Complete';
  } else {
    return 'Pending ' + event.seconds_until_confirmed + 's';
  }
}

export default App;
