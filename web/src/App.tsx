import { Suspense, lazy } from 'react';
import { Navigate, Route, Routes } from 'react-router';

import { Layout } from './components/Layout';
import { AnalysisPage } from './features/analysis/AnalysisPage';
import { PlayPage } from './features/play/PlayPage';

// Pages not needed at startup (the training one pulls in the charts library):
// load them on demand.
const TrainingPage = lazy(() =>
  import('./features/training/TrainingPage').then((module) => ({ default: module.TrainingPage })),
);
const ArenaPage = lazy(() =>
  import('./features/arena/ArenaPage').then((module) => ({ default: module.ArenaPage })),
);

const loading = <p className="text-sm text-dim">Chargement…</p>;

export function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Navigate to="/play" replace />} />
        <Route path="/play" element={<PlayPage />} />
        <Route path="/analysis" element={<AnalysisPage />} />
        <Route
          path="/training"
          element={
            <Suspense fallback={loading}>
              <TrainingPage />
            </Suspense>
          }
        />
        <Route
          path="/arena"
          element={
            <Suspense fallback={loading}>
              <ArenaPage />
            </Suspense>
          }
        />
        <Route path="*" element={<Navigate to="/play" replace />} />
      </Route>
    </Routes>
  );
}
