import { useCallback, useEffect, useRef, useState } from 'react';

import { ApiError, api } from './client';
import type { AnalysisInfo } from './generated/AnalysisInfo';
import type { ArenaView } from './generated/ArenaView';
import type { EngineLimits } from './generated/EngineLimits';
import type { GameRequest } from './generated/GameRequest';
import type { HealthResponse } from './generated/HealthResponse';
import type { PositionView } from './generated/PositionView';
import type { ServerMessage } from './generated/ServerMessage';
import type { TrainingView } from './generated/TrainingView';
import { type ServerSocket, type SocketStatus, connectServerSocket } from './socket';

export type HealthState =
  { kind: 'loading' } | { kind: 'ok'; health: HealthResponse } | { kind: 'error'; message: string };

const HEALTH_POLL_MS = 5_000;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'erreur inconnue';
}

/** Polls `GET /api/health`. */
export function useHealth(): HealthState {
  const [state, setState] = useState<HealthState>({ kind: 'loading' });

  useEffect(() => {
    let controller = new AbortController();
    const poll = () => {
      controller.abort();
      controller = new AbortController();
      api
        .health(controller.signal)
        .then((health) => {
          setState({ kind: 'ok', health });
        })
        .catch((error: unknown) => {
          if (controller.signal.aborted) return;
          setState({ kind: 'error', message: errorMessage(error) });
        });
    };
    poll();
    const timer = setInterval(poll, HEALTH_POLL_MS);
    return () => {
      clearInterval(timer);
      controller.abort();
    };
  }, []);

  return state;
}

/** Connects to `/ws` for the lifetime of the component and exposes its status. */
export function useServerSocket(onMessage?: (message: ServerMessage) => void): {
  status: SocketStatus;
  socket: ServerSocket | null;
} {
  const [status, setStatus] = useState<SocketStatus>('connecting');
  const [socket, setSocket] = useState<ServerSocket | null>(null);
  // Kept in a ref so a new callback identity does not reopen the socket.
  const onMessageRef = useRef(onMessage);
  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    const connection = connectServerSocket({
      onStatus: setStatus,
      onMessage: (message) => onMessageRef.current?.(message),
    });
    setSocket(connection);
    return () => {
      connection.close();
      setSocket(null);
    };
  }, []);

  return { status, socket };
}

export type PositionState = {
  /** Latest successfully loaded view (kept while the next one loads). */
  view: PositionView | null;
  /** Why the latest request failed (e.g. illegal move in an imported game). */
  error: string | null;
  loading: boolean;
};

/** Keeps the server's view of `game` up to date. */
export function usePosition(game: GameRequest): PositionState {
  const key = JSON.stringify(game);
  // `key` records which request the stored result answers.
  const [result, setResult] = useState<{
    key: string;
    view: PositionView | null;
    error: string | null;
  }>({ key: '', view: null, error: null });

  useEffect(() => {
    const controller = new AbortController();
    api
      .position(JSON.parse(key) as GameRequest, controller.signal)
      .then((view) => {
        setResult({ key, view, error: null });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        setResult((previous) => ({ key, view: previous.view, error: errorMessage(error) }));
      });
    return () => {
      controller.abort();
    };
  }, [key]);

  const loading = result.key !== key;
  return { view: result.view, error: loading ? null : result.error, loading };
}

export type AnalysisState = {
  socket: SocketStatus;
  /** Whether an analysis is running. */
  running: boolean;
  /** Latest completed depth of the current analysis. */
  info: AnalysisInfo | null;
  error: string | null;
};

/**
 * Live analysis over the WebSocket. `analyze` replaces any analysis in
 * progress; messages of replaced analyses are ignored thanks to their id.
 */
export function useAnalysis(): AnalysisState & {
  analyze: (game: GameRequest, limits?: EngineLimits) => void;
  stop: () => void;
} {
  const currentId = useRef(0);
  const [state, setState] = useState<Omit<AnalysisState, 'socket'>>({
    running: false,
    info: null,
    error: null,
  });

  const onMessage = useCallback((message: ServerMessage) => {
    switch (message.type) {
      case 'info':
        if (message.info.id === currentId.current) {
          setState((previous) => ({ ...previous, info: message.info }));
        }
        break;
      case 'done':
        if (message.id === currentId.current) {
          setState((previous) => ({ ...previous, running: false }));
        }
        break;
      case 'error':
        setState((previous) => ({ ...previous, running: false, error: message.message }));
        break;
      default:
        break;
    }
  }, []);

  const { status, socket } = useServerSocket(onMessage);

  const analyze = useCallback(
    (game: GameRequest, limits: EngineLimits = {}) => {
      if (!socket) return;
      currentId.current += 1;
      setState({ running: true, info: null, error: null });
      socket.send({ type: 'analyze', id: currentId.current, game, limits });
    },
    [socket],
  );

  const stop = useCallback(() => {
    socket?.send({ type: 'stop' });
  }, [socket]);

  return { ...state, socket: status, analyze, stop };
}

export { ApiError };

const TRAINING_POLL_MS = 3_000;

export type Polled<T> = {
  view: T | null;
  error: string | null;
  /** Fetches again right away (after a command). */
  refresh: () => void;
};

/** Polls `fetcher` every `intervalMs`, again whenever `key` changes. */
function usePolling<T>(
  key: string,
  fetcher: (signal: AbortSignal) => Promise<T>,
  intervalMs: number,
): Polled<T> {
  // While another key loads, the previous view stays (better than a blank page).
  const [state, setState] = useState<{ view: T | null; error: string | null }>({
    view: null,
    error: null,
  });
  const [tick, setTick] = useState(0);
  // The fetcher is rebuilt on every render: keep the latest without re-polling.
  const fetcherRef = useRef(fetcher);
  useEffect(() => {
    fetcherRef.current = fetcher;
  }, [fetcher]);

  useEffect(() => {
    const controller = new AbortController();
    const poll = () => {
      fetcherRef
        .current(controller.signal)
        .then((view) => {
          setState({ view, error: null });
        })
        .catch((error: unknown) => {
          if (controller.signal.aborted) return;
          setState((previous) => ({ ...previous, error: errorMessage(error) }));
        });
    };
    poll();
    const timer = setInterval(poll, intervalMs);
    return () => {
      clearInterval(timer);
      controller.abort();
    };
  }, [key, tick, intervalMs]);

  const refresh = useCallback(() => {
    setTick((value) => value + 1);
  }, []);
  return { ...state, refresh };
}

/** Polls `GET /api/training` for `network` (default: the one training). */
export function useTraining(network: string | null): Polled<TrainingView> {
  return usePolling(network ?? '', (signal) => api.training(network, signal), TRAINING_POLL_MS);
}

/** Polls `GET /api/arena`. */
export function useArena(): Polled<ArenaView> {
  return usePolling('arena', (signal) => api.arena(signal), TRAINING_POLL_MS);
}
