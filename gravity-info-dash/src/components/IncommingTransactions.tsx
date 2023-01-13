import { Table, Card, CardBody, CardTitle } from 'reactstrap';
import { DepositWithMetadata, Erc20Metadata, EthInfo } from '../types';
import {
  amountToFraction,
  cosmosAddressToExplorerLink,
  getMetadataFromList,
  printTxStatus
} from '../utils';

interface Props {
  ethBridgeInfo: EthInfo;
  etherscanBase: string;
  erc20Metadata: Array<Erc20Metadata>;
}

export const IncommingTransactions: React.FC<Props> = ({
  ethBridgeInfo,
  etherscanBase,
  erc20Metadata
}) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 20 }}>
      <CardBody>
        <CardTitle tag="h1">Incoming transactions</CardTitle>
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
                      getMetadataFromList(sendToCosmos.erc20, erc20Metadata)
                        ?.symbol
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
                    <a href={`${etherscanBase}/address/${sendToCosmos.sender}`}>
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
                  <td>{printTxStatus(sendToCosmos)}</td>
                </tr>
              )
            )}
          </tbody>
        </Table>
      </CardBody>
    </Card>
  );
};
