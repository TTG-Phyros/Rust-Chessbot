import { Suspense, lazy } from 'react';
import { Navigate, Route, Routes } from 'react-router';

import { Layout } from './components/Layout';
import { AnalysisPage } from './features/analysis/AnalysisPage';
import { PlayPage } from './features/play/PlayPage';

// The charts library is only needed here: load it on demand.
const TrainingPage = lazy(() =>
  import('./features/training/TrainingPage').then((module) => ({ default: module.TrainingPage })),
);

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
            <Suspense fallback={<p className="text-sm text-slate-400">Chargement…</p>}>
              <TrainingPage />
            </Suspense>
          }
        />
        <Route path="*" element={<Navigate to="/play" replace />} />
      </Route>
    </Routes>
  );
}
