/**
 * Typed REST client. All HTTP traffic of the UI goes through this module.
 *
 * Paths are relative (`/api/...`): Nginx (Docker) or Vite (dev) proxies them
 * to `chess-server`, so the UI and the API share one origin.
 */
import type { HealthResponse } from './generated/HealthResponse';

/** Error raised when the server answers with a non-2xx status. */
export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  headers.set('Accept', 'application/json');
  const response = await fetch(`/api${path}`, { ...init, headers });
  if (!response.ok) {
    throw new ApiError(response.status, await response.text());
  }
  // The shape is guaranteed by the server: these types are generated from its Rust structs.
  return (await response.json()) as T;
}

export const api = {
  health: (signal?: AbortSignal) => request<HealthResponse>('/health', { signal: signal ?? null }),
};
