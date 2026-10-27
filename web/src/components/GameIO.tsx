import { useState } from 'react';

import { api } from '../api/client';
import type { GameRequest } from '../api/generated/GameRequest';
import type { PgnTag } from '../api/generated/PgnTag';
import { newGame } from '../lib/game';
import { Button, ErrorText, INPUT } from './ui';

type GameIOProps = {
  game: GameRequest;
  /** FEN of the position on the board. */
  fen: string | null;
  /** Tags written in exported PGN (players, event...). */
  tags?: PgnTag[];
  onLoad: (game: GameRequest) => void;
};

/** Import and export of FEN and PGN. The server validates every import. */
export function GameIO({ game, fen, tags = [], onLoad }: GameIOProps) {
  const [fenInput, setFenInput] = useState('');
  const [pgn, setPgn] = useState('');
  const [message, setMessage] = useState<{ error: boolean; text: string } | null>(null);

  const run = async (action: () => Promise<string>) => {
    try {
      setMessage({ error: false, text: await action() });
    } catch (error: unknown) {
      setMessage({ error: true, text: error instanceof Error ? error.message : 'erreur' });
    }
  };

  const loadFen = () =>
    run(async () => {
      const candidate = newGame(fenInput.trim());
      await api.position(candidate); // throws on an invalid FEN
      onLoad(candidate);
      return 'Position chargée.';
    });

  const importPgn = () =>
    run(async () => {
      const imported = await api.importPgn(pgn);
      onLoad(imported.game);
      return `Partie importée (${imported.game.moves.length} coups).`;
    });

  const exportPgn = () =>
    run(async () => {
      const exported = await api.exportPgn({ game, tags });
      setPgn(exported.pgn);
      download('partie.pgn', exported.pgn);
      return 'PGN exporté (et téléchargé).';
    });

  const copyFen = () =>
    run(async () => {
      if (!fen) throw new Error('aucune position');
      await navigator.clipboard.writeText(fen);
      return 'FEN copiée dans le presse-papiers.';
    });

  return (
    <div className="space-y-4 text-sm">
      <div className="flex gap-2">
        <input
          value={fenInput}
          onChange={(event) => {
            setFenInput(event.target.value);
          }}
          placeholder={fen ?? 'FEN'}
          aria-label="FEN"
          className={`${INPUT} min-w-0 flex-1 text-xs`}
        />
        <Button onClick={() => void loadFen()} disabled={!fenInput.trim()}>
          Charger
        </Button>
        <Button onClick={() => void copyFen()} variant="ghost">
          Copier
        </Button>
      </div>
      <textarea
        value={pgn}
        onChange={(event) => {
          setPgn(event.target.value);
        }}
        placeholder="Coller un PGN ici…"
        aria-label="PGN"
        rows={7}
        className={`${INPUT} text-xs`}
      />
      <div className="flex gap-2">
        <Button variant="primary" onClick={() => void importPgn()} disabled={!pgn.trim()}>
          Importer PGN
        </Button>
        <Button onClick={() => void exportPgn()}>Exporter PGN</Button>
      </div>
      {message &&
        (message.error ? (
          <ErrorText>{message.text}</ErrorText>
        ) : (
          <p className="text-signal">{message.text}</p>
        ))}
    </div>
  );
}

function download(filename: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/x-chess-pgn' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
