import React, { useEffect, useState } from 'react';
import './App.css';
import {
  Spinner,
  CardBody,
  CardTitle,
  Card,
  CardSubtitle,
} from "reactstrap";
import { BatchFees, ChainTotalSupplyNumbers, Erc20Metadata, EthInfo, GravityInfo } from './types';

// 5 seconds
const UPDATE_TIME = 5000;

const BACKEND_PORT = 9000;
export const SERVER_URL =
  "http://" + window.location.hostname + ":" + BACKEND_PORT + "/";

function App() {
  document.title = "Gravity Bridge Info"
  const [gravityBridgeInfo, setGravityBridgeInfo] = useState<GravityInfo | null>(null);
  const [ethBridgeInfo, setEthBridgeInfo] = useState<EthInfo | null>(null);
  const [supplyInfo, setSupplyInfo] = useState<ChainTotalSupplyNumbers | null>(null);
  const [erc20Metadata, setErc20Metadata] = useState<Array<Erc20Metadata> | null>(null);

  async function getGravityInfo() {
    let request_url = SERVER_URL + "gravity_bridge_info";
    const requestOptions: any = {
      method: "GET",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
    };

    const result = await fetch(request_url, requestOptions);
    const json = await result.json();
    setGravityBridgeInfo(json)
  }
  async function getEthInfo() {
    let request_url = SERVER_URL + "eth_bridge_info";
    const requestOptions: any = {
      method: "GET",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
    };

    const result = await fetch(request_url, requestOptions);
    const json = await result.json();
    setEthBridgeInfo(json)
  }
  async function getDistributionInfo() {
    let request_url = SERVER_URL + "supply_info";
    const requestOptions: any = {
      method: "GET",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
    };

    const result = await fetch(request_url, requestOptions);
    const json = await result.json();
    setSupplyInfo(json)
  }
  async function getErc20Metadata() {
    let request_url = SERVER_URL + "erc20_metadata";
    const requestOptions: any = {
      method: "GET",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
    };

    const result = await fetch(request_url, requestOptions);
    const json = await result.json();
    setErc20Metadata(json)
  }


  useEffect(() => {
    getDistributionInfo();
    getGravityInfo();
    getEthInfo();
    getErc20Metadata();
    //eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    const interval = setInterval(() => {
      getDistributionInfo();
      getGravityInfo();
      getEthInfo();
      getErc20Metadata();
    }, UPDATE_TIME);
    return () => clearInterval(interval);
    //eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (gravityBridgeInfo == null || ethBridgeInfo == null || supplyInfo == null || erc20Metadata == null) {
    return (
      <div className="App-header" style={{ display: "flex", flexWrap: "wrap" }}>
        <Spinner
          color="danger"
          type="grow"
        >
          Loading...
        </Spinner>
      </div>
    )
  }

  let bridge_address = gravityBridgeInfo.params.bridge_ethereum_address;
  let etherscanLink = "https://etherscan.io/" + bridge_address;

  return (
    <div className="App-header" style={{ display: "flex", flexWrap: "wrap" }}>
      <div style={{ padding: 5 }}>
        <Card className="ParametersCard" style={{ borderRadius: 8, padding: 20 }}>
          <CardBody>
            <CardTitle tag="h4">
              Batch Queue Info
            </CardTitle>
            <CardSubtitle>Lists the number of transactions and total fee amount waiting for relay to Ethereum</CardSubtitle>
            {
              gravityBridgeInfo.pending_tx.map((batch_fees: BatchFees) => (
                <>
                  <div>
                    {batch_fees.total_fees}
                  </div>
                </>
              ))
            }

          </CardBody>
        </Card>
      </div>
      <div style={{ padding: 5 }}>
        <Card className="ParametersCard" style={{ borderRadius: 8, padding: 25 }}>
          <CardBody>
            <CardTitle tag="h4">
              Current Gravity Parameters
            </CardTitle>
            Ethereum Contract Address: <a href={etherscanLink}>{bridge_address}</a>
          </CardBody>
        </Card>
      </div>

    </div >
  );
}

export default App;
