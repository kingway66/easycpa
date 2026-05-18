import { useState, useCallback } from 'react'
import { createContext, useContext } from 'react'

export interface ModelRoute {
  name: string
  model: string
  base_url: string
  api_key: string
  api_format: string
  context_window?: number
  max_output_tokens?: number
  default_reasoning_level?: string
  supported_reasoning_levels?: string[]
  supports_parallel_tool_calls?: boolean
  supports_reasoning_summaries?: boolean
  enabled?: boolean
}

export interface ClaudeSettingsFile {
  filename: string
  env: Record<string, string>
}

export interface CodexModelProvider {
  name: string
  base_url: string
  wire_api: string
  requires_openai_auth: boolean
}

export interface CodexProfile {
  name: string
  model_provider?: string
  model?: string
  model_reasoning_effort?: string
  preferred_auth_method?: string
  model_context_window?: number
  model_auto_compact_token_limit?: number
  approvals_reviewer?: string
}

export interface CodexConfig {
  model_providers: CodexModelProvider[]
  profiles: CodexProfile[]
}

interface ApiContextType {
  // Model routes
  models: ModelRoute[]
  loading: boolean
  fetchModels: () => Promise<void>
  saveModel: (model: ModelRoute) => Promise<void>
  deleteModel: (name: string, base_url?: string, model?: string) => Promise<void>

  // Claude settings
  claudeSettings: ClaudeSettingsFile[]
  fetchClaudeSettings: () => Promise<void>
  saveClaudeSettings: (filename: string, env: Record<string, string>) => Promise<void>
  deleteClaudeSettings: (filename: string) => Promise<void>

  // Codex config
  codexConfig: CodexConfig | null
  fetchCodexConfig: () => Promise<void>
  saveCodexModelProvider: (provider: CodexModelProvider) => Promise<void>
  deleteCodexModelProvider: (name: string) => Promise<void>
  saveCodexProfile: (profile: CodexProfile) => Promise<void>
  deleteCodexProfile: (name: string) => Promise<void>
}

const ApiContext = createContext<ApiContextType | null>(null)

export function useApi() {
  const ctx = useContext(ApiContext)
  if (!ctx) throw new Error('useApi must be used within ApiProvider')
  return ctx
}

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

export function ApiProvider({ children }: { children: React.ReactNode }) {
  const [models, setModels] = useState<ModelRoute[]>([])
  const [claudeSettings, setClaudeSettings] = useState<ClaudeSettingsFile[]>([])
  const [codexConfig, setCodexConfig] = useState<CodexConfig | null>(null)
  const [loading, setLoading] = useState(false)

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

  return (
    <ApiContext.Provider value={{
      models, loading, fetchModels, saveModel, deleteModel,
      claudeSettings, fetchClaudeSettings, saveClaudeSettings, deleteClaudeSettings,
      codexConfig, fetchCodexConfig, saveCodexModelProvider, deleteCodexModelProvider, saveCodexProfile, deleteCodexProfile,
    }}>
      {children}
    </ApiContext.Provider>
  )
}
