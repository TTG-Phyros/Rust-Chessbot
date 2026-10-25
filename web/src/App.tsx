import { Navigate, Route, Routes } from 'react-router';

import { Layout } from './components/Layout';
import { AnalysisPage } from './features/analysis/AnalysisPage';
import { PlayPage } from './features/play/PlayPage';
import { TrainingPage } from './features/training/TrainingPage';

export function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Navigate to="/play" replace />} />
        <Route path="/play" element={<PlayPage />} />
        <Route path="/analysis" element={<AnalysisPage />} />
        <Route path="/training" element={<TrainingPage />} />
        <Route path="*" element={<Navigate to="/play" replace />} />
      </Route>
    </Routes>
  );
}
