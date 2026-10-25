import { Chessboard } from 'react-chessboard';

import { Placeholder } from '../../components/Placeholder';

export function PlayPage() {
  return (
    <Placeholder title="Jouer contre le bot" phase={4}>
      <div className="max-w-md">
        <Chessboard options={{ position: 'start', allowDragging: false }} />
      </div>
    </Placeholder>
  );
}
