import { Card, CardBody, CardSubtitle, CardTitle, Table } from 'reactstrap';
import {
  BatchTransaction,
  Erc20Metadata,
  EthInfo,
  GravityInfo,
  TransactionBatch
} from '../types';
import {
  amountToFraction,
  cosmosAddressToExplorerLink,
  getMetadataFromList,
  getNotExecutedBatches
} from '../utils';

interface Props {
  gravityBridgeInfo: GravityInfo;
  erc20Metadata: Array<Erc20Metadata>;
  ethBridgeInfo: EthInfo;
  etherscanBase: string;
  evmChainPrefix?: string;
}

const prefixToName = (prefix?: string) => {
  switch (prefix) {
    case 'oraib':
      return 'Binance Smart Chain';
    default:
      return 'Ethereum Mainnet';
  }
};

export const BatchQueue: React.FC<Props> = ({
  gravityBridgeInfo,
  erc20Metadata,
  ethBridgeInfo,
  etherscanBase,
  evmChainPrefix
}) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 20 }}>
      <CardBody>
        <CardTitle tag="h1">Batch Queue</CardTitle>
        <CardSubtitle>
          These transactions are in batches and waiting to be relayed to
          <strong> {prefixToName(evmChainPrefix)}</strong>
        </CardSubtitle>
        {getNotExecutedBatches(gravityBridgeInfo, ethBridgeInfo).map(
          (batch: TransactionBatch) => (
            <div key={batch.nonce}>
              <h5>
                {' '}
                Batch #{batch.nonce}{' '}
                {
                  getMetadataFromList(batch.token_contract, erc20Metadata)
                    ?.symbol
                }
              </h5>
              <div>
                Total Fees:{' '}
                {amountToFraction(
                  batch.token_contract,
                  batch.total_fee.amount,
                  erc20Metadata
                )}
              </div>
              <div className="mb-4">
                Timeout:{' '}
                <a href={`${etherscanBase}/block/${batch.batch_timeout}`}>
                  {batch.batch_timeout}
                </a>
              </div>
              <Table
                dark
                borderless
                size="sm"
                responsive
                style={{ borderSpacing: 10, fontSize: 15 }}
              >
                <thead>
                  <tr>
                    <th>To</th>
                    <th>From</th>
                    <th>Amount / Fee</th>
                  </tr>
                </thead>
                <tbody>
                  {batch.transactions.map((batchTx: BatchTransaction) => (
                    <tr key={batchTx.id}>
                      <td>
                        <a href={etherscanBase + batchTx.destination}>
                          {batchTx.destination}
                        </a>
                      </td>
                      <td>
                        <a href={cosmosAddressToExplorerLink(batchTx.sender)}>
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
                  ))}
                </tbody>
              </Table>
            </div>
          )
        )}
      </CardBody>
    </Card>
  );
};
