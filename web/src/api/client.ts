/**
 * Typed REST client. All HTTP traffic of the UI goes through this module.
 *
 * Paths are relative (`/api/...`): Nginx (Docker) or Vite (dev) proxies them
 * to `chess-server`, so the UI and the API share one origin.
 */
import type { EngineMoveRequest } from './generated/EngineMoveRequest';
import type { EngineMoveResponse } from './generated/EngineMoveResponse';
import type { ErrorResponse } from './generated/ErrorResponse';
import type { GameRequest } from './generated/GameRequest';
import type { HealthResponse } from './generated/HealthResponse';
import type { PgnExportRequest } from './generated/PgnExportRequest';
import type { PgnExportResponse } from './generated/PgnExportResponse';
import type { PgnImportResponse } from './generated/PgnImportResponse';
import type { PositionView } from './generated/PositionView';

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
};
