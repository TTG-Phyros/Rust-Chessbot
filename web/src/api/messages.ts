import type { ServerMessage } from './generated/ServerMessage';

const SERVER_MESSAGE_TYPES: ReadonlySet<string> = new Set<ServerMessage['type']>([
  'hello',
  'pong',
  'error',
  'info',
  'done',
]);

/**
 * Parses a WebSocket frame sent by the server.
 *
 * Returns `null` for anything that is not a known {@link ServerMessage}, so a
 * newer server can add message kinds without crashing an older UI.
 */
export function parseServerMessage(raw: string): ServerMessage | null {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (
    typeof value === 'object' &&
    value !== null &&
    'type' in value &&
    typeof value.type === 'string' &&
    SERVER_MESSAGE_TYPES.has(value.type)
  ) {
    return value as ServerMessage;
  }
  return null;
}
