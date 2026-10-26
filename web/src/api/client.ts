/**
 * Typed REST client. All HTTP traffic of the UI goes through this module.
 *
 * Paths are relative (`/api/...`): Nginx (Docker) or Vite (dev) proxies them
 * to `chess-server`, so the UI and the API share one origin.
 */
import type { ArenaView } from './generated/ArenaView';
import type { BotInfo } from './generated/BotInfo';
import type { CreateNetworkRequest } from './generated/CreateNetworkRequest';
import type { EngineMoveRequest } from './generated/EngineMoveRequest';
import type { EngineMoveResponse } from './generated/EngineMoveResponse';
import type { ErrorResponse } from './generated/ErrorResponse';
import type { GameRequest } from './generated/GameRequest';
import type { HealthResponse } from './generated/HealthResponse';
import type { NetworkParams } from './generated/NetworkParams';
import type { PgnExportRequest } from './generated/PgnExportRequest';
import type { PgnExportResponse } from './generated/PgnExportResponse';
import type { PgnImportResponse } from './generated/PgnImportResponse';
import type { PositionView } from './generated/PositionView';
import type { SaveBotRequest } from './generated/SaveBotRequest';
import type { StartTrainingRequest } from './generated/StartTrainingRequest';
import type { TournamentRequest } from './generated/TournamentRequest';
import type { TournamentStarted } from './generated/TournamentStarted';
import type { TrainingView } from './generated/TrainingView';

/** Error raised when the server answers with a non-2xx status. */
export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

function isErrorResponse(value: unknown): value is ErrorResponse {
  return typeof value === 'object' && value !== null && 'error' in value;
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Accept', 'application/json');
  const response = await fetch(`/api${path}`, { ...init, headers });
  if (!response.ok) {
    const body: unknown = await response.json().catch(() => null);
    const message = isErrorResponse(body) ? body.error : `HTTP ${response.status}`;
    throw new ApiError(response.status, message);
  }
  // The shape is guaranteed by the server: these types are generated from its Rust structs.
  return (await response.json()) as T;
}

function post<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
  return request<T>(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal: signal ?? null,
  });
}

export const api = {
  health: (signal?: AbortSignal) => request<HealthResponse>('/health', { signal: signal ?? null }),

  /** Replays a game: legal moves, status, history. Rejects illegal moves (422). */
  position: (game: GameRequest, signal?: AbortSignal) =>
    post<PositionView>('/position', game, signal),

  /** The engine's move in a game. */
  engineMove: (body: EngineMoveRequest, signal?: AbortSignal) =>
    post<EngineMoveResponse>('/engine/move', body, signal),

  /** First game of a PGN text. */
  importPgn: (pgn: string) => post<PgnImportResponse>('/pgn/import', { pgn }),

  /** A game as PGN text. */
  exportPgn: (body: PgnExportRequest) => post<PgnExportResponse>('/pgn/export', body),

  /**
   * Training progress of a network (default: the one training), and the
   * list of every network.
   */
  training: (network: string | null, signal?: AbortSignal) =>
    request<TrainingView>(
      network ? `/training?network=${encodeURIComponent(network)}` : '/training',
      { signal: signal ?? null },
    ),

  /** Starts the self-play + training pipeline of a network (409 if one is running). */
  startTraining: (body: StartTrainingRequest) => command('POST', '/training/start', body),

  /** Stops the pipeline after its current step (it checkpoints first). */
  stopTraining: () => command('POST', '/training/stop', {}),

  /** Hyperparameters of a new network (the training service's settings). */
  networkDefaults: () => request<NetworkParams>('/networks/defaults'),

  /** Creates a network (409 if the name is taken, 422 if a parameter is invalid). */
  createNetwork: (body: CreateNetworkRequest) => command('POST', '/networks', body),

  /** Deletes a network's settings, checkpoint and weights. */
  deleteNetwork: (name: string) => command('DELETE', `/networks/${encodeURIComponent(name)}`),

  /** Bots and tournaments. */
  arena: (signal?: AbortSignal) => request<ArenaView>('/arena', { signal: signal ?? null }),

  /** Saves a snapshot (or the latest export) of a network as a bot. */
  saveBot: (body: SaveBotRequest) => post<BotInfo>('/bots', body),

  deleteBot: (name: string) => command('DELETE', `/bots/${encodeURIComponent(name)}`),

  /** Starts a round-robin tournament (409 if one is in progress). */
  startTournament: (body: TournamentRequest) => post<TournamentStarted>('/tournaments', body),

  /** Stops the tournament in progress. */
  stopTournament: () => command('POST', '/tournaments/stop', {}),
};

/** A command answered with an empty body (201, 202 or 204). */
async function command(method: 'POST' | 'DELETE', path: string, body?: unknown): Promise<void> {
  const response = await fetch(`/api${path}`, {
    method,
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: body === undefined ? null : JSON.stringify(body),
  });
  if (!response.ok) {
    const error: unknown = await response.json().catch(() => null);
    throw new ApiError(
      response.status,
      isErrorResponse(error) ? error.error : `HTTP ${response.status}`,
    );
  }
}
