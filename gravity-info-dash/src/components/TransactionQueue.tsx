import { Card, CardBody, CardSubtitle, CardTitle, Table } from 'reactstrap';
import { BatchFees, Erc20Metadata, GravityInfo } from '../types';
import { amountToFraction, getMetadataFromList } from '../utils';

interface Props {
  gravityBridgeInfo: GravityInfo;
  erc20Metadata: Array<Erc20Metadata>;
}

export const TransactionQueue: React.FC<Props> = ({
  gravityBridgeInfo,
  erc20Metadata
}) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 20 }}>
      <CardBody>
        <CardTitle tag="h1">Transaction Queue</CardTitle>
        <CardSubtitle>
          These transactions are not yet in batches, a batch will be reqested
          when the fee amount exceeds the cost to execute on Ethereum
        </CardSubtitle>
        <Table
          dark
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
            {gravityBridgeInfo.pending_tx.map((batchFees: BatchFees, ind) => (
              <tr key={ind}>
                <td>
                  {getMetadataFromList(batchFees.token, erc20Metadata)?.symbol}
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
            ))}
          </tbody>
        </Table>
      </CardBody>
    </Card>
  );
};
