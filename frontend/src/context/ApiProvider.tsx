import { useCallback, useState } from 'react'
import type { ReactNode } from 'react'
import {
  ApiContext,
  type ClaudeSettingsFile,
  type CodexConfig,
  type CodexModelProvider,
  type CodexProfile,
  type ModelRoute,
} from './api-context'

async function api(path: string, opts?: RequestInit) {
  const res = await fetch(`/api${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...opts,
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(`API error ${res.status}: ${text}`)
  }
  if (res.status === 204) return
  return res.json()
}

export function ApiProvider({ children }: { children: ReactNode }) {
  const [models, setModels] = useState<ModelRoute[]>([])
  const [claudeSettings, setClaudeSettings] = useState<ClaudeSettingsFile[]>([])
  const [codexConfig, setCodexConfig] = useState<CodexConfig | null>(null)
  const [loading, setLoading] = useState(false)
  const [configPath, setConfigPath] = useState('')
  const [version, setVersion] = useState('')

  const fetchConfigPath = useCallback(async () => {
    const res = await fetch('/status')
    if (!res.ok) return

    const data = await res.json()
    setConfigPath(data.config_path || '')
    setVersion(data.version || '')
  }, [])

  const fetchModels = useCallback(async () => {
    setLoading(true)
    try {
      const data = await api('/models')
      setModels(data.models || [])
    } finally {
      setLoading(false)
    }
  }, [])

  const saveModel = useCallback(async (model: ModelRoute) => {
    await api(`/models/${encodeURIComponent(model.name)}`, {
      method: 'PUT',
      body: JSON.stringify(model),
    })
    await fetchModels()
  }, [fetchModels])

  const deleteModel = useCallback(async (name: string, base_url?: string, model?: string) => {
    let path = `/models/${encodeURIComponent(name)}`
    if (base_url && model) {
      path += `?base_url=${encodeURIComponent(base_url)}&model=${encodeURIComponent(model)}`
    }
    await api(path, { method: 'DELETE' })
    await fetchModels()
  }, [fetchModels])

  const fetchClaudeSettings = useCallback(async () => {
    setLoading(true)
    try {
      const data = await api('/claude-settings')
      setClaudeSettings(data.files || [])
    } finally {
      setLoading(false)
    }
  }, [])

  const saveClaudeSettings = useCallback(async (filename: string, env: Record<string, string>) => {
    await api(`/claude-settings/${encodeURIComponent(filename)}`, {
      method: 'PUT',
      body: JSON.stringify({ env }),
    })
    await fetchClaudeSettings()
  }, [fetchClaudeSettings])

  const deleteClaudeSettings = useCallback(async (filename: string) => {
    await api(`/claude-settings/${encodeURIComponent(filename)}`, { method: 'DELETE' })
    await fetchClaudeSettings()
  }, [fetchClaudeSettings])

  const fetchCodexConfig = useCallback(async () => {
    setLoading(true)
    try {
      const data = await api('/codex-config')
      setCodexConfig(data)
    } finally {
      setLoading(false)
    }
  }, [])

  const saveCodexModelProvider = useCallback(async (provider: CodexModelProvider) => {
    await api('/codex-config/providers', {
      method: 'PUT',
      body: JSON.stringify(provider),
    })
    await fetchCodexConfig()
  }, [fetchCodexConfig])

  const deleteCodexModelProvider = useCallback(async (name: string) => {
    await api(`/codex-config/providers/${encodeURIComponent(name)}`, { method: 'DELETE' })
    await fetchCodexConfig()
  }, [fetchCodexConfig])

  const saveCodexProfile = useCallback(async (profile: CodexProfile) => {
    await api('/codex-config/profiles', {
      method: 'PUT',
      body: JSON.stringify(profile),
    })
    await fetchCodexConfig()
  }, [fetchCodexConfig])

  const deleteCodexProfile = useCallback(async (name: string) => {
    await api(`/codex-config/profiles/${encodeURIComponent(name)}`, { method: 'DELETE' })
    await fetchCodexConfig()
  }, [fetchCodexConfig])

  const serverReload = useCallback(async () => {
    await api('/server/reload', { method: 'POST' })
  }, [])

  const serverRestart = useCallback(async () => {
    await api('/server/restart', { method: 'POST' })
  }, [])

  return (
    <ApiContext.Provider value={{
      configPath, version, fetchConfigPath, models, loading, fetchModels, saveModel, deleteModel,
      claudeSettings, fetchClaudeSettings, saveClaudeSettings, deleteClaudeSettings,
      codexConfig, fetchCodexConfig, saveCodexModelProvider, deleteCodexModelProvider, saveCodexProfile, deleteCodexProfile,
      serverReload, serverRestart,
    }}>
      {children}
    </ApiContext.Provider>
  )
}

