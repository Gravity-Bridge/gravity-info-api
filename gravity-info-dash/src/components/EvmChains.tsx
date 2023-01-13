import { Button, ButtonGroup, Card, CardBody, CardTitle } from 'reactstrap';
import { EvmChainConfig } from '../types';

interface Props {
  configs: Array<EvmChainConfig>;
  evmChainPrefix?: string;
  onSelect: (config: EvmChainConfig) => void;
}

export const EvmChains: React.FC<Props> = ({
  configs,
  onSelect,
  evmChainPrefix
}) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 20 }}>
      <CardBody>
        <CardTitle tag="h1">Evm chains</CardTitle>
        <ButtonGroup size="sm">
          {configs.map((config) => (
            <Button
              outline
              color="primary"
              title={config.rpc}
              active={config.prefix === evmChainPrefix}
              key={config.prefix}
              onClick={() => {
                onSelect(config);
              }}
            >
              {config.prefix}
            </Button>
          ))}
        </ButtonGroup>
      </CardBody>
    </Card>
  );
};
