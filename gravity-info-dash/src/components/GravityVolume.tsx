import { Card, CardBody, CardTitle } from 'reactstrap';
import { VolumeInfo } from '../types';

interface Props {
  volumeInfo: VolumeInfo;
}

const decimals = 10 ** 6;
export const GravityVolume: React.FC<Props> = ({ volumeInfo }) => {
  return (
    <Card className="ParametersCard" style={{ borderRadius: 8, padding: 25 }}>
      <CardBody>
        <CardTitle tag="h1">Gravity Volume</CardTitle>
        <div>
          Daily Volume ${(volumeInfo.daily_volume / decimals).toFixed(2)}M
        </div>
        <div>
          Daily Inflow ${(volumeInfo.daily_inflow / decimals).toFixed(2)}M
        </div>
        <div>
          Daily Outflow ${(volumeInfo.daily_outflow / decimals).toFixed(2)}M
        </div>
        <div>
          Weekly Volume ${(volumeInfo.weekly_volume / decimals).toFixed(2)}M
        </div>
        <div>
          Weekly Inflow ${(volumeInfo.weekly_inflow / decimals).toFixed(2)}M
        </div>
        <div>
          Weekly Outflow ${(volumeInfo.weekly_outflow / decimals).toFixed(2)}M
        </div>
      </CardBody>
    </Card>
  );
};
