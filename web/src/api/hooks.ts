import { useEffect, useRef, useState } from 'react';

import { api } from './client';
import type { HealthResponse } from './generated/HealthResponse';
import type { ServerMessage } from './generated/ServerMessage';
import { connectServerSocket, type SocketStatus } from './socket';

export type HealthState =
  { kind: 'loading' } | { kind: 'ok'; health: HealthResponse } | { kind: 'error'; message: string };

const HEALTH_POLL_MS = 5_000;

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
          setState({ kind: 'error', message: error instanceof Error ? error.message : 'unknown' });
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
export function useServerSocket(onMessage?: (message: ServerMessage) => void): SocketStatus {
  const [status, setStatus] = useState<SocketStatus>('connecting');
  // Kept in a ref so a new callback identity does not reopen the socket.
  const onMessageRef = useRef(onMessage);
  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    const socket = connectServerSocket({
      onStatus: setStatus,
      onMessage: (message) => onMessageRef.current?.(message),
    });
    return () => {
      socket.close();
    };
  }, []);

  return status;
}
