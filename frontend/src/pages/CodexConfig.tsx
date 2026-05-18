import { useEffect, useState } from 'react'
import { Copy, Check, ChevronDown, ChevronRight } from 'lucide-react'
import { useApi } from '../context/ApiContext'
import type { CodexModelProvider, CodexProfile } from '../context/ApiContext'

function generateToml(providers: CodexModelProvider[], profiles: CodexProfile[]) {
  let toml = ''
  for (const p of providers) {
    toml += `[model_providers.${p.name}]\n`
    toml += `name = "${p.name}"\n`
    toml += `base_url = "${p.base_url}"\n`
    toml += `wire_api = "${p.wire_api}"\n`
    toml += `requires_openai_auth = ${p.requires_openai_auth}\n`
    toml += '\n'
  }
  for (const p of profiles) {
    toml += `[profiles.${p.name}]\n`
    if (p.model_provider) toml += `model_provider = "${p.model_provider}"\n`
    if (p.model) toml += `model = "${p.model}"\n`
    if (p.model_reasoning_effort) toml += `model_reasoning_effort = "${p.model_reasoning_effort}"\n`
    if (p.preferred_auth_method) toml += `preferred_auth_method = "${p.preferred_auth_method}"\n`
    if (p.model_context_window) toml += `model_context_window = ${p.model_context_window}\n`
    if (p.model_auto_compact_token_limit) toml += `model_auto_compact_token_limit = ${p.model_auto_compact_token_limit}\n`
    if (p.approvals_reviewer) toml += `approvals_reviewer = "${p.approvals_reviewer}"\n`
    toml += '\n'
  }
  return toml.trim()
}

export default function CodexConfig() {
  const { codexConfig, loading, fetchCodexConfig } = useApi()
  const [expanded, setExpanded] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => { fetchCodexConfig() }, [fetchCodexConfig])

  const providers = codexConfig?.model_providers || []
  const profiles = codexConfig?.profiles || []
  const toml = generateToml(providers, profiles)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(toml)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div>
      <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 mb-6">
        <p className="text-sm text-amber-800">
          本页面仅辅助生成配置，你需要自己写配置文件。
        </p>
      </div>

      <h2 className="text-xl font-semibold text-gray-900 mb-6">Codex Config</h2>
      {loading && <p className="text-sm text-gray-500">Loading...</p>}

      {/* Model Providers summary */}
      <div className="mb-4">
        <button
          onClick={() => setExpanded(expanded === 'providers' ? null : 'providers')}
          className="flex items-center gap-2 text-sm font-medium text-gray-700 hover:text-gray-900 mb-2"
        >
          {expanded === 'providers' ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          Model Providers ({providers.length})
        </button>
        {expanded === 'providers' && (
          <div className="space-y-1 ml-6">
            {providers.map(p => (
              <div key={p.name} className="text-xs text-gray-600 py-1">
                <span className="font-medium">{p.name}</span>
                <span className="text-gray-400"> · {p.base_url} · {p.wire_api}</span>
              </div>
            ))}
            {providers.length === 0 && <p className="text-xs text-gray-400">None</p>}
          </div>
        )}
      </div>

      {/* Profiles summary */}
      <div className="mb-4">
        <button
          onClick={() => setExpanded(expanded === 'profiles' ? null : 'profiles')}
          className="flex items-center gap-2 text-sm font-medium text-gray-700 hover:text-gray-900 mb-2"
        >
          {expanded === 'profiles' ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          Profiles ({profiles.length})
        </button>
        {expanded === 'profiles' && (
          <div className="space-y-1 ml-6">
            {profiles.map(p => (
              <div key={p.name} className="text-xs text-gray-600 py-1">
                <span className="font-medium">{p.name}</span>
                <span className="text-gray-400">
                  {p.model_provider && ` · provider: ${p.model_provider}`}
                  {p.model && ` · model: ${p.model}`}
                  {p.model_reasoning_effort && ` · ${p.model_reasoning_effort}`}
                </span>
              </div>
            ))}
            {profiles.length === 0 && <p className="text-xs text-gray-400">None</p>}
          </div>
        )}
      </div>

      {/* TOML output */}
      <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
        <div className="flex items-center justify-between px-4 py-2.5 bg-gray-50 border-b border-gray-100">
          <span className="text-xs text-gray-500">~/.codex/config.toml</span>
          <button
            onClick={handleCopy}
            className="inline-flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
          >
            {copied ? <Check size={12} className="text-green-500" /> : <Copy size={12} />}
            {copied ? 'Copied' : 'Copy'}
          </button>
        </div>
        <pre className="text-xs text-gray-800 p-4 overflow-x-auto whitespace-pre">
          {toml || '# No providers or profiles configured'}
        </pre>
      </div>
    </div>
  )
}
