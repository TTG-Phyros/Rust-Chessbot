import { useHealth, useServerSocket } from '../api/hooks';

function Dot({ ok }: { ok: boolean }) {
  return (
    <span
      aria-hidden
      className={`inline-block size-2 rounded-full ${ok ? 'bg-emerald-400' : 'bg-rose-500'}`}
    />
  );
}

/** Shows whether the REST API and the WebSocket are reachable. */
export function ServerStatus() {
  const health = useHealth();
  const { status: socket } = useServerSocket();

  const apiLabel =
    health.kind === 'ok'
      ? `API v${health.health.version}`
      : health.kind === 'loading'
        ? 'API…'
        : 'API hors ligne';

  return (
    <div className="flex items-center gap-4 text-xs text-slate-400" data-testid="server-status">
      <span
        className="flex items-center gap-1.5"
        title={health.kind === 'error' ? health.message : ''}
      >
        <Dot ok={health.kind === 'ok'} />
        {apiLabel}
      </span>
      <span className="flex items-center gap-1.5">
        <Dot ok={socket === 'open'} />
        {socket === 'open' ? 'WebSocket' : `WebSocket (${socket})`}
      </span>
    </div>
  );
}
