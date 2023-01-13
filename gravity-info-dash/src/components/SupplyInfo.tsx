import { Card, CardBody, CardTitle } from 'reactstrap';
import { ChainTotalSupplyNumbers } from '../types';
import { DENOM } from '../utils';

interface Props {
  supplyInfo: ChainTotalSupplyNumbers;
}

export const SupplyInfo: React.FC<Props> = ({ supplyInfo }) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 25 }}>
      <CardBody>
        <CardTitle tag="h1">Gravity Supply Info</CardTitle>
        <div>
          Total Supply: {(supplyInfo.total_supply / 10 ** 12).toFixed(2)}M{' '}
          {DENOM}
        </div>
        <div>
          Community Pool: {(supplyInfo.community_pool / 10 ** 12).toFixed(2)}M{' '}
          {DENOM}
        </div>
        <div>
          Liquid (Not Vesting):{' '}
          {(supplyInfo.total_liquid_supply / 10 ** 12).toFixed(2)}M {DENOM}
        </div>
        <div>
          Liquid (Not Vesting) and staked:{' '}
          {(supplyInfo.total_nonvesting_staked / 10 ** 12).toFixed(2)}M {DENOM}
        </div>
        <div>
          Unclaimed staking rewards:{' '}
          {(supplyInfo.total_unclaimed_rewards / 10 ** 12).toFixed(2)}M {DENOM}
        </div>
        <div>
          Unvested: {(supplyInfo.total_vesting / 10 ** 12).toFixed(2)}M {DENOM}
        </div>
        <div>
          Unvested Staked:{' '}
          {(supplyInfo.total_vesting_staked / 10 ** 12).toFixed(2)}M {DENOM}
        </div>
        <div>
          Vested: {(supplyInfo.total_vested / 10 ** 12).toFixed(2)}M {DENOM}
        </div>
      </CardBody>
    </Card>
  );
};
