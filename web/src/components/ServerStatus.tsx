import type { ControllerState } from '../api/generated/ControllerState';
import { useController, useHealth, useServerSocket } from '../api/hooks';
import { StatusPill, type Tone } from './ui';

const TRAINING: Record<ControllerState, { tone: Tone; label: string }> = {
  unavailable: { tone: 'alert', label: 'Entraînement hors ligne' },
  idle: { tone: 'idle', label: 'Entraînement en attente' },
  running: { tone: 'think', label: 'Entraînement en cours' },
  stopping: { tone: 'warn', label: 'Entraînement : arrêt…' },
};

/** Shows whether the REST API, the WebSocket and the training service are reachable. */
export function ServerStatus() {
  const health = useHealth();
  const { status: socket } = useServerSocket();
  const controller = useController();

  const apiLabel =
    health.kind === 'ok'
      ? `API v${health.health.version}`
      : health.kind === 'loading'
        ? 'API…'
        : 'API hors ligne';
  const training = controller.view ? TRAINING[controller.view.state] : null;

  return (
    <div className="flex flex-wrap items-center gap-2">
      <StatusPill
        tone={health.kind === 'ok' ? 'signal' : health.kind === 'loading' ? 'idle' : 'alert'}
        title={health.kind === 'error' ? health.message : undefined}
      >
        {apiLabel}
      </StatusPill>
      <StatusPill tone={socket === 'open' ? 'signal' : socket === 'connecting' ? 'warn' : 'alert'}>
        {socket === 'open' ? 'WebSocket' : `WebSocket (${socket})`}
      </StatusPill>
      {training && (
        <StatusPill
          tone={training.tone}
          title={controller.view?.error ?? controller.view?.run ?? undefined}
        >
          {training.label}
        </StatusPill>
      )}
    </div>
  );
}
