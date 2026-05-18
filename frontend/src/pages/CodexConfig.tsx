import { useEffect, useState } from 'react'
import { Copy, Check, Plus, Eye, Trash2 } from 'lucide-react'
import { useApi } from '../context/ApiContext'
import type { CodexModelProvider, CodexProfile } from '../context/ApiContext'

const EMPTY_PROVIDER: CodexModelProvider = { name: '', base_url: '', wire_api: 'responses', requires_openai_auth: true }
const EMPTY_PROFILE: CodexProfile = { name: '', model_provider: '', model: '', model_reasoning_effort: 'high', preferred_auth_method: 'apikey' }

function generateToml(providers: CodexModelProvider[], profiles: CodexProfile[]) {
  let toml = ''
  for (const p of providers) {
    toml += `[model_providers.${p.name}]\n`
    toml += `name = "${p.name}"\n`
    toml += `base_url = "${p.base_url}"\n`
    toml += `wire_api = "${p.wire_api}"\n`
    toml += `requires_openai_auth = ${p.requires_openai_auth}\n\n`
  }
  for (const p of profiles) {
    toml += `[profiles.${p.name}]\n`
    if (p.model_provider) toml += `model_provider = "${p.model_provider}"\n`
    if (p.model) toml += `model = "${p.model}"\n`
    if (p.model_reasoning_effort) toml += `model_reasoning_effort = "${p.model_reasoning_effort}"\n`
    if (p.preferred_auth_method) toml += `preferred_auth_method = "${p.preferred_auth_method}"\n`
    if (p.model_context_window) toml += `model_context_window = ${p.model_context_window}\n`
    if (p.model_auto_compact_token_limit) toml += `model_auto_compact_token_limit = ${p.model_auto_compact_token_limit}\n`
    if (p.approvals_reviewer) toml += `approvals_reviewer = "${p.approvals_reviewer}"\n\n`
  }
  return toml.trim()
}

export default function CodexConfig() {
  const { codexConfig, loading, fetchCodexConfig } = useApi()
  const [genProviders, setGenProviders] = useState<CodexModelProvider[]>([])
  const [genProfiles, setGenProfiles] = useState<CodexProfile[]>([])
  const [copied, setCopied] = useState(false)
  const [viewing, setViewing] = useState(false)

  useEffect(() => { fetchCodexConfig() }, [fetchCodexConfig])

  const existingProviders = codexConfig?.model_providers || []
  const existingProfiles = codexConfig?.profiles || []

  const generatedToml = generateToml(genProviders, genProfiles)
  const hasContent = genProviders.length > 0 || genProfiles.length > 0

  const handleCopy = async (text: string) => {
    await navigator.clipboard.writeText(text)
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

      {/* Generator: Model Providers */}
      <div className="bg-white border border-gray-200 rounded-lg p-5 mb-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Plus size={16} className="text-blue-600" />
            <h3 className="text-sm font-medium text-gray-900">生成 Model Providers</h3>
          </div>
          <button
            onClick={() => setGenProviders(p => [...p, { ...EMPTY_PROVIDER }])}
            className="inline-flex items-center gap-1 px-2 py-1 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700"
          >
            <Plus size={12} /> Add
          </button>
        </div>
        <div className="space-y-3">
          {genProviders.map((p, i) => (
            <div key={i} className="border border-gray-100 rounded-md p-3 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-gray-500">Provider #{i + 1}</span>
                <button onClick={() => setGenProviders(prev => prev.filter((_, j) => j !== i))}
                  className="text-red-400 hover:text-red-600"><Trash2 size={12} /></button>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Name</label>
                  <input value={p.name} onChange={e => setGenProviders(prev => prev.map((v, j) => j === i ? { ...v, name: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Base URL</label>
                  <input value={p.base_url} onChange={e => setGenProviders(prev => prev.map((v, j) => j === i ? { ...v, base_url: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
                </div>
              </div>
              <div className="flex items-center gap-4">
                <div className="flex-1">
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Wire API</label>
                  <select value={p.wire_api} onChange={e => setGenProviders(prev => prev.map((v, j) => j === i ? { ...v, wire_api: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
                    <option value="responses">responses</option>
                    <option value="chat">chat</option>
                  </select>
                </div>
                <label className="flex items-center gap-2 text-sm">
                  <input type="checkbox" checked={p.requires_openai_auth}
                    onChange={e => setGenProviders(prev => prev.map((v, j) => j === i ? { ...v, requires_openai_auth: e.target.checked } : v))} />
                  OpenAI Auth
                </label>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Generator: Profiles */}
      <div className="bg-white border border-gray-200 rounded-lg p-5 mb-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Plus size={16} className="text-blue-600" />
            <h3 className="text-sm font-medium text-gray-900">生成 Profiles</h3>
          </div>
          <button
            onClick={() => setGenProfiles(p => [...p, { ...EMPTY_PROFILE }])}
            className="inline-flex items-center gap-1 px-2 py-1 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700"
          >
            <Plus size={12} /> Add
          </button>
        </div>
        <div className="space-y-3">
          {genProfiles.map((p, i) => (
            <div key={i} className="border border-gray-100 rounded-md p-3 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-gray-500">Profile #{i + 1}</span>
                <button onClick={() => setGenProfiles(prev => prev.filter((_, j) => j !== i))}
                  className="text-red-400 hover:text-red-600"><Trash2 size={12} /></button>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Name</label>
                  <input value={p.name} onChange={e => setGenProfiles(prev => prev.map((v, j) => j === i ? { ...v, name: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Model Provider</label>
                  <input value={p.model_provider || ''} onChange={e => setGenProfiles(prev => prev.map((v, j) => j === i ? { ...v, model_provider: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Model</label>
                  <input value={p.model || ''} onChange={e => setGenProfiles(prev => prev.map((v, j) => j === i ? { ...v, model: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
                </div>
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Reasoning Effort</label>
                  <select value={p.model_reasoning_effort || ''} onChange={e => setGenProfiles(prev => prev.map((v, j) => j === i ? { ...v, model_reasoning_effort: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
                    <option value="">default</option>
                    <option value="low">low</option>
                    <option value="medium">medium</option>
                    <option value="high">high</option>
                    <option value="xhigh">xhigh</option>
                  </select>
                </div>
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-0.5">Auth Method</label>
                  <select value={p.preferred_auth_method || ''} onChange={e => setGenProfiles(prev => prev.map((v, j) => j === i ? { ...v, preferred_auth_method: e.target.value } : v))}
                    className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
                    <option value="">default</option>
                    <option value="apikey">apikey</option>
                    <option value="oauth">oauth</option>
                  </select>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Generated TOML output */}
      {hasContent && (
        <div className="bg-white border border-blue-200 rounded-lg overflow-hidden mb-6">
          <div className="flex items-center justify-between px-4 py-2.5 bg-blue-50 border-b border-blue-100">
            <span className="text-xs font-medium text-blue-700">生成结果</span>
            <button
              onClick={() => handleCopy(generatedToml)}
              className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
            >
              {copied ? <Check size={12} /> : <Copy size={12} />}
              {copied ? '已复制' : '复制'}
            </button>
          </div>
          <pre className="text-xs text-gray-800 p-4 overflow-x-auto whitespace-pre">
            {generatedToml}
          </pre>
        </div>
      )}

      {/* Existing config — read only */}
      <div>
        <div className="flex items-center gap-2 mb-3">
          <Eye size={14} className="text-gray-400" />
          <h3 className="text-sm font-medium text-gray-700">已有配置（仅供参考）</h3>
          <button
            onClick={() => setViewing(!viewing)}
            className="text-xs text-blue-600 hover:text-blue-700"
          >
            {viewing ? '收起' : '查看'}
          </button>
        </div>
        {loading && <p className="text-sm text-gray-500">Loading...</p>}
        {viewing && !loading && (
          <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
            <div className="flex items-center justify-between px-4 py-2.5 bg-gray-50 border-b border-gray-100">
              <span className="text-xs text-gray-500">~/.codex/config.toml</span>
              <button
                onClick={() => handleCopy(generateToml(existingProviders, existingProfiles))}
                className="inline-flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
              >
                <Copy size={12} /> 复制
              </button>
            </div>
            <pre className="text-xs text-gray-800 p-4 overflow-x-auto whitespace-pre">
              {generateToml(existingProviders, existingProfiles) || '# No providers or profiles configured'}
            </pre>
          </div>
        )}
      </div>
    </div>
  )
}