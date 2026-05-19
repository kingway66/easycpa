import { Routes, Route, Navigate } from 'react-router-dom'
import Layout from './components/Layout'
import Dashboard from './pages/Dashboard'
import ModelDetail from './pages/ModelDetail'
import ClaudeSettings from './pages/ClaudeSettings'
import CodexConfig from './pages/CodexConfig'

export default function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Dashboard />} />
        <Route path="/dashboard" element={<Dashboard />} />
        <Route path="/models/:name" element={<ModelDetail />} />
        <Route path="/models" element={<Navigate to="/models/gpt-5.5" replace />} />
        <Route path="/claude-settings" element={<ClaudeSettings />} />
        <Route path="/codex-config" element={<CodexConfig />} />
      </Route>
    </Routes>
  )
}
