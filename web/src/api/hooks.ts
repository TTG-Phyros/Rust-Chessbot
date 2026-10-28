import { useCallback, useEffect, useRef, useState } from 'react';

import { api } from './client';
import type { AnalysisInfo } from './generated/AnalysisInfo';
import type { ArenaView } from './generated/ArenaView';
import type { BotInfo } from './generated/BotInfo';
import type { ControllerView } from './generated/ControllerView';
import type { EngineInfo } from './generated/EngineInfo';
import type { EngineLimits } from './generated/EngineLimits';
import type { GameRequest } from './generated/GameRequest';
import type { HealthResponse } from './generated/HealthResponse';
import type { PositionView } from './generated/PositionView';
import type { ReviewEstimates } from './generated/ReviewEstimates';
import type { ReviewGameSummary } from './generated/ReviewGameSummary';
import type { ReviewGameView } from './generated/ReviewGameView';
import type { ReviewJobView } from './generated/ReviewJobView';
import type { ReviewOptions } from './generated/ReviewOptions';
import type { ReviewProfile } from './generated/ReviewProfile';
import type { ReviewRequest } from './generated/ReviewRequest';
import type { SelfplayView } from './generated/SelfplayView';
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
  /** Every completed depth of the current analysis, shallowest first. */
  depths: AnalysisInfo[];
  /** The game being analysed, which `info` and `depths` describe. */
  game: GameRequest | null;
  error: string | null;
};

/**
 * Live analysis over the WebSocket. `analyze` replaces any analysis in
 * progress; messages of replaced analyses are ignored thanks to their id.
 */
export function useAnalysis(): AnalysisState & {
  analyze: (game: GameRequest, limits?: EngineLimits, bot?: string | null) => void;
  stop: () => void;
} {
  const currentId = useRef(0);
  const [state, setState] = useState<Omit<AnalysisState, 'socket'>>({
    running: false,
    info: null,
    depths: [],
    game: null,
    error: null,
  });

  const onMessage = useCallback((message: ServerMessage) => {
    switch (message.type) {
      case 'info':
        if (message.info.id === currentId.current) {
          setState((previous) => ({
            ...previous,
            info: message.info,
            depths: [
              ...previous.depths.filter((info) => info.depth < message.info.depth),
              message.info,
            ],
          }));
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
    (game: GameRequest, limits: EngineLimits = {}, bot: string | null = null) => {
      if (!socket) return;
      currentId.current += 1;
      setState({ running: true, info: null, depths: [], game, error: null });
      socket.send({ type: 'analyze', id: currentId.current, game, limits, bot });
    },
    [socket],
  );

  const stop = useCallback(() => {
    socket?.send({ type: 'stop' });
  }, [socket]);

  return { ...state, socket: status, analyze, stop };
}

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

const CONTROLLER_POLL_MS = 10_000;

/** Polls the state of the training service (for the header). */
export function useController(): Polled<ControllerView> {
  return usePolling(
    'controller',
    (signal) => api.training(null, signal).then((view) => view.controller),
    CONTROLLER_POLL_MS,
  );
}

const SELFPLAY_POLL_MS = 1_000;

/** Polls the boards of the self-play workers (every second: they move fast). */
export function useSelfplay(): Polled<SelfplayView> {
  return usePolling('selfplay', (signal) => api.selfplay(signal), SELFPLAY_POLL_MS);
}

const BOTS_POLL_MS = 30_000;

/** Polls the saved bots. */
export function useBots(): Polled<BotInfo[]> {
  return usePolling('bots', (signal) => api.bots(signal), BOTS_POLL_MS);
}

const ENGINE_POLL_MS = 30_000;

/** Polls which evaluation the engine plays with (the server reloads the network). */
export function useEngineInfo(): Polled<EngineInfo> {
  return usePolling('engine', (signal) => api.engine(signal), ENGINE_POLL_MS);
}

export type Fetched<T> = {
  /** The answer for the current key; `null` while it loads or after an error. */
  view: T | null;
  error: string | null;
  loading: boolean;
};

/** Fetches once per `key` (`null`: nothing to fetch). */
function useFetched<T>(
  key: string | null,
  fetcher: (key: string, signal: AbortSignal) => Promise<T>,
): Fetched<T> {
  const [result, setResult] = useState<{ key: string; view: T | null; error: string | null }>({
    key: '',
    view: null,
    error: null,
  });
  const fetcherRef = useRef(fetcher);
  useEffect(() => {
    fetcherRef.current = fetcher;
  }, [fetcher]);

  useEffect(() => {
    if (key === null) return;
    const controller = new AbortController();
    fetcherRef
      .current(key, controller.signal)
      .then((view) => {
        setResult({ key, view, error: null });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        setResult({ key, view: null, error: errorMessage(error) });
      });
    return () => {
      controller.abort();
    };
  }, [key]);

  const current = key !== null && result.key === key;
  return {
    view: current ? result.view : null,
    error: current ? result.error : null,
    loading: key !== null && !current,
  };
}

/** A cached Chess.com game, its moves classified; fetched again when `version` changes. */
export function useReviewGame(id: string | null, version = 0): Fetched<ReviewGameView> {
  const key = id === null ? null : JSON.stringify([id, version]);
  return useFetched(key, (json, signal) => {
    const [game] = JSON.parse(json) as [string];
    return api.reviewGame(game, signal);
  });
}

/**
 * The cached Chess.com games of a player kept by `query` (see `filtersQuery`),
 * most recent first; fetched again when `version` changes.
 */
export function useReviewGames(
  username: string | null,
  query: string,
  version = 0,
): Fetched<ReviewGameSummary[]> {
  const key = username === null ? null : JSON.stringify([username, query, version]);
  return useFetched(key, (json, signal) => {
    const [name, filters] = JSON.parse(json) as [string, string];
    return api.reviewGames(name, filters, signal);
  });
}

/** Profile of a player over the games kept by `query`. */
export function useReviewProfile(
  username: string | null,
  query: string,
  version = 0,
): Fetched<ReviewProfile> {
  const key = username === null ? null : JSON.stringify([username, query, version]);
  return useFetched(key, (json, signal) => {
    const [name, filters] = JSON.parse(json) as [string, string];
    return api.reviewProfile(name, filters, signal);
  });
}

/** Presets and limits of the reviews. */
export function useReviewOptions(): Fetched<ReviewOptions> {
  return useFetched('options', (_, signal) => api.reviewOptions(signal));
}

/** Duration of the review of `request` with each preset. */
export function useReviewEstimate(
  request: ReviewRequest | null,
  version = 0,
): Fetched<ReviewEstimates> {
  const key = request === null ? null : JSON.stringify([request, version]);
  return useFetched(key, (json, signal) => {
    const [body] = JSON.parse(json) as [ReviewRequest];
    return api.reviewEstimate(body, signal);
  });
}

/**
 * Progress of review job `id`, streamed by `GET /api/review/jobs/{id}/events`
 * (Server-Sent Events) until the job ends.
 */
export function useReviewJob(id: number | null): ReviewJobView | null {
  const [state, setState] = useState<{ id: number; view: ReviewJobView } | null>(null);
  useEffect(() => {
    if (id === null) return;
    const source = new EventSource(`/api/review/jobs/${String(id)}/events`);
    source.addEventListener('progress', (event) => {
      setState({ id, view: JSON.parse(String(event.data)) as ReviewJobView });
    });
    // The stream ends with the job; an error before that: read it once.
    source.onerror = () => {
      source.close();
      api
        .reviewJob(id)
        .then((view) => {
          setState({ id, view });
        })
        .catch(() => {
          // The page keeps the latest progress received.
        });
    };
    return () => {
      source.close();
    };
  }, [id]);
  return state?.id === id ? state.view : null;
}
