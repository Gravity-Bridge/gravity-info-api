import { Card, CardBody, CardTitle } from 'reactstrap';
import { EvmChainParam, GravityInfo } from '../types';

interface Props {
  gravityBridgeInfo: GravityInfo;
  evmChainParam?: EvmChainParam;
  etherscanBase: string;
}

export const CurrentParameters: React.FC<Props> = ({
  gravityBridgeInfo,
  evmChainParam,

  etherscanBase
}) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 25 }}>
      <CardBody>
        <CardTitle tag="h1">Current Gravity Parameters</CardTitle>
        <div>
          Ethereum Contract Address:{' '}
          {evmChainParam?.bridge_ethereum_address && (
            <a
              href={`${etherscanBase}/address/${evmChainParam.bridge_ethereum_address}`}
            >
              {evmChainParam.bridge_ethereum_address}
            </a>
          )}
        </div>
        <div>Bridge Active: {String(evmChainParam?.bridge_active)}</div>
        <div>
          Target Batch Timeout:{' '}
          {gravityBridgeInfo.params.target_batch_timeout / 1000 / (60 * 60)}{' '}
          hours
        </div>
      </CardBody>
    </Card>
  );
};
