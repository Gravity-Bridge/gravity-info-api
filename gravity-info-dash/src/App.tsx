import React, { useEffect, useState } from 'react';
import 'bootstrap/dist/css/bootstrap.min.css';
import './App.css';
import { Spinner, Container, Col, Row } from 'reactstrap';
import {
  ChainTotalSupplyNumbers,
  Erc20Metadata,
  EthInfo,
  GravityInfo,
  VolumeInfo,
  EvmChainConfig
} from './types';
import { EvmChains } from './components/EvmChains';
import { IncommingTransactions } from './components/IncommingTransactions';
import { getEtherScanBase } from './utils';
import { GravityVolume } from './components/GravityVolume';
import { TransactionQueue } from './components/TransactionQueue';
import { BatchQueue } from './components/BatchQueue';
import { CurrentParameters } from './components/CurrentParameters';
import { SupplyInfo } from './components/SupplyInfo';

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

  const [evmChainPrefix, setEvmChainPrefix] = useState<string | undefined>();

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
    let timer: any = null;
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
        timer = setTimeout(update, UPDATE_TIME);
      };
      update();
    }
    return () => clearTimeout(timer);
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
      <Container className="App" fluid>
        <Spinner color="primary" type="grow">
          Loading...
        </Spinner>
      </Container>
    );
  }

  const evmChainParam = gravityBridgeInfo.params.evm_chain_params.find(
    (p) => p.evm_chain_prefix === evmChainPrefix
  );

  const etherscanBase = getEtherScanBase(evmChainPrefix);

  return (
    <Container className="App" fluid>
      <Row>
        <Col>
          <BatchQueue
            etherscanBase={etherscanBase}
            gravityBridgeInfo={gravityBridgeInfo}
            erc20Metadata={erc20Metadata}
            ethBridgeInfo={ethBridgeInfo}
          />

          <IncommingTransactions
            ethBridgeInfo={ethBridgeInfo}
            etherscanBase={etherscanBase}
            erc20Metadata={erc20Metadata}
          />
        </Col>
        <Col>
          <EvmChains
            configs={evmChainConfigs}
            evmChainPrefix={evmChainPrefix}
            onSelect={(config: EvmChainConfig) => {
              setEvmChainPrefix(config.prefix);
            }}
          />

          <TransactionQueue
            gravityBridgeInfo={gravityBridgeInfo}
            erc20Metadata={erc20Metadata}
          />

          <CurrentParameters
            gravityBridgeInfo={gravityBridgeInfo}
            evmChainParam={evmChainParam}
            etherscanBase={etherscanBase}
          />

          <SupplyInfo supplyInfo={supplyInfo} />

          <GravityVolume volumeInfo={volumeInfo} />
        </Col>
      </Row>
    </Container>
  );
}

export default App;
