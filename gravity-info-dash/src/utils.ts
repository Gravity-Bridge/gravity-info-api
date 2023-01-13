/// Inefficient utility function to lookup token metadata, should be using a map

import {
  Attestation,
  DepositWithMetadata,
  Erc20Metadata,
  EthInfo,
  GravityInfo
} from './types';

/// of some kind
export function getMetadataFromList(
  erc20: string,
  metadata: Array<Erc20Metadata>
) {
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
export function amountToFraction(
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
export function getNotExecutedBatches(
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
export function alreadyExecuted(batch_nonce: number, ethBridgeInfo: EthInfo) {
  var arrayLength = ethBridgeInfo.batch_events.length;
  for (var i = 0; i < arrayLength; i++) {
    if (ethBridgeInfo.batch_events[i].batch_nonce === batch_nonce) {
      return true;
    }
  }
  return false;
}

export const DENOM = 'uoraib';

const prefixBaseMap: { [key: string]: string } = {
  gravity: 'https://mintscan.io/gravity-bridge/account/',
  osmosis: 'https://mintscan.io/osmosis/account/',
  cre: 'https://mintscan.io/crescent/account/',
  canto: 'https://explorer.nodestake.top/canto/account/',
  mantle: 'https://mintscan.io/mantle/account/',
  orai: 'https://scan.orai.io/account/',
  oraib: 'https://scan.bridge.orai.io/account/'
};
/// Takes various cosmos addresses to create a proper mintscan link
export const cosmosAddressToExplorerLink = (input: string): string => {
  const prefix = input.split('1')[0];
  return prefixBaseMap[prefix] || input;
};

export const getEtherScanBase = (evmChainPrefix?: string): string => {
  switch (evmChainPrefix) {
    case 'oraib':
      return 'https://bscscan.com';
    default:
      return 'https://etherscan.io';
  }
};

// takes a send to Cosmos event and determines its status
export const printTxStatus = (event: DepositWithMetadata) => {
  if (event.confirmed) {
    return 'Complete';
  } else {
    return 'Pending ' + event.seconds_until_confirmed + 's';
  }
};
