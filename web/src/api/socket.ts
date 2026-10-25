import type { ClientMessage } from './generated/ClientMessage';
import type { ServerMessage } from './generated/ServerMessage';
import { parseServerMessage } from './messages';

export type SocketStatus = 'connecting' | 'open' | 'closed';

export interface SocketHandlers {
  onMessage: (message: ServerMessage) => void;
  onStatus: (status: SocketStatus) => void;
}

export interface ServerSocket {
  /** Sends a message; silently dropped while disconnected. */
  send: (message: ClientMessage) => void;
  /** Closes the socket and stops reconnecting. */
  close: () => void;
}

const MAX_BACKOFF_MS = 10_000;

/** Opens `/ws` on the current origin and reconnects with exponential backoff. */
export function connectServerSocket(handlers: SocketHandlers): ServerSocket {
  let socket: WebSocket | null = null;
  let closedByUser = false;
  let attempt = 0;
  let retryTimer: ReturnType<typeof setTimeout> | undefined;

  const url = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`;

  const open = () => {
    handlers.onStatus('connecting');
    const ws = new WebSocket(url);
    socket = ws;
    ws.onopen = () => {
      attempt = 0;
      handlers.onStatus('open');
    };
    ws.onmessage = (event: MessageEvent<unknown>) => {
      if (typeof event.data !== 'string') return;
      const message = parseServerMessage(event.data);
      if (message) handlers.onMessage(message);
    };
    ws.onclose = () => {
      socket = null;
      handlers.onStatus('closed');
      if (closedByUser) return;
      const delay = Math.min(MAX_BACKOFF_MS, 500 * 2 ** attempt);
      attempt += 1;
      retryTimer = setTimeout(open, delay);
    };
  };

  open();

  return {
    send: (message) => {
      if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
    },
    close: () => {
      closedByUser = true;
      clearTimeout(retryTimer);
      socket?.close();
    },
  };
}
