import { createContext, useContext } from 'react'

export interface ModelRoute {
  name: string
  model: string
  base_url: string
  api_key: string
  proxy_url?: string
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

export interface ApiContextType {
  configPath: string
  version: string
  fetchConfigPath: () => Promise<void>
  models: ModelRoute[]
  loading: boolean
  fetchModels: () => Promise<void>
  saveModel: (model: ModelRoute) => Promise<void>
  deleteModel: (name: string, base_url?: string, model?: string) => Promise<void>
  claudeSettings: ClaudeSettingsFile[]
  fetchClaudeSettings: () => Promise<void>
  saveClaudeSettings: (filename: string, env: Record<string, string>) => Promise<void>
  deleteClaudeSettings: (filename: string) => Promise<void>
  codexConfig: CodexConfig | null
  fetchCodexConfig: () => Promise<void>
  saveCodexModelProvider: (provider: CodexModelProvider) => Promise<void>
  deleteCodexModelProvider: (name: string) => Promise<void>
  saveCodexProfile: (profile: CodexProfile) => Promise<void>
  deleteCodexProfile: (name: string) => Promise<void>
  serverReload: () => Promise<void>
  serverRestart: () => Promise<void>
}

export const ApiContext = createContext<ApiContextType | null>(null)

export function useApi() {
  const ctx = useContext(ApiContext)
  if (!ctx) throw new Error('useApi must be used within ApiProvider')
  return ctx
}

