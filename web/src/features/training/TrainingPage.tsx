import { Placeholder } from '../../components/Placeholder';

export function TrainingPage() {
  return (
    <Placeholder title="Entraînement" phase={6}>
      <p className="text-sm text-slate-300">
        Courbes de loss, Elo entre checkpoints, device utilisé, lancement et arrêt des sessions.
      </p>
    </Placeholder>
  );
}
