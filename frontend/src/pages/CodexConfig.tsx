import { useEffect, useState } from 'react'
import { Copy, Check, Eye } from 'lucide-react'
import { useApi } from '../context/ApiContext'
import type { CodexModelProvider, CodexProfile } from '../context/ApiContext'

const EASYCPA_PROVIDER_PRESET: CodexModelProvider = {
  name: 'easycpa',
  base_url: 'http://localhost:15791/v1',
  wire_api: 'responses',
  requires_openai_auth: true,
}
const EASYCPA_PROFILE_PRESET: CodexProfile = {
  name: 'easycpa',
  model_provider: 'easycpa',
  model: 'gpt-5.5',
  model_reasoning_effort: 'xhigh',
  preferred_auth_method: 'apikey',
  model_context_window: 300000,
  model_auto_compact_token_limit: 270000,
  approvals_reviewer: 'user',
}

const AUTH_JSON_SAMPLE = JSON.stringify(
  { OPENAI_API_KEY: 'PROXY_KEY_ANY_STRING' },
  null,
  2,
)

function generateToml(provider: CodexModelProvider, profile: CodexProfile) {
  let toml = `[model_providers.${provider.name}]\n`
  toml += `name = "${provider.name}"\n`
  toml += `base_url = "${provider.base_url}"\n`
  toml += `wire_api = "${provider.wire_api}"\n`
  toml += `requires_openai_auth = ${provider.requires_openai_auth}\n\n`
  toml += `[profiles.${profile.name}]\n`
  if (profile.model_provider) toml += `model_provider = "${profile.model_provider}"\n`
  if (profile.model) toml += `model = "${profile.model}"\n`
  if (profile.model_reasoning_effort) toml += `model_reasoning_effort = "${profile.model_reasoning_effort}"\n`
  if (profile.preferred_auth_method) toml += `preferred_auth_method = "${profile.preferred_auth_method}"\n`
  if (profile.model_context_window) toml += `model_context_window = ${profile.model_context_window}\n`
  if (profile.model_auto_compact_token_limit) toml += `model_auto_compact_token_limit = ${profile.model_auto_compact_token_limit}\n`
  if (profile.approvals_reviewer) toml += `approvals_reviewer = "${profile.approvals_reviewer}"\n`
  return toml.trim()
}

function generateTomlMulti(providers: CodexModelProvider[], profiles: CodexProfile[]) {
  let toml = ''
  for (const p of providers) {
    toml += `[model_providers.${p.name}]\nname = "${p.name}"\nbase_url = "${p.base_url}"\nwire_api = "${p.wire_api}"\nrequires_openai_auth = ${p.requires_openai_auth}\n\n`
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
  const [provider, setProvider] = useState<CodexModelProvider>({ ...EASYCPA_PROVIDER_PRESET })
  const [profile, setProfile] = useState<CodexProfile>({ ...EASYCPA_PROFILE_PRESET })
  const [copied, setCopied] = useState(false)
  const [authCopied, setAuthCopied] = useState(false)
  const [viewing, setViewing] = useState(false)

  useEffect(() => { fetchCodexConfig() }, [fetchCodexConfig])

  const existingProviders = codexConfig?.model_providers || []
  const existingProfiles = codexConfig?.profiles || []

  const generatedToml = generateToml(provider, profile)

  const handleCopy = async (text: string, setter: (v: boolean) => void) => {
    await navigator.clipboard.writeText(text)
    setter(true)
    setTimeout(() => setter(false), 2000)
  }

  return (
    <div>
      <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 mb-6">
        <p className="text-sm text-amber-800">
          本页面仅辅助生成配置，你需要自己写配置文件。
        </p>
        <p className="text-sm text-amber-700 mt-1">
          例如写入 <code className="bg-amber-100 px-1 rounded">~/.codex/config.toml</code>，然后用 <code className="bg-amber-100 px-1 rounded">codex --profile easycpa</code> 启动。
        </p>
        <p className="text-sm text-amber-700 mt-1">
          还需要写入 <code className="bg-amber-100 px-1 rounded">~/.codex/auth.json</code>（请先备份原来的 auth.json）。
        </p>
      </div>

      <h2 className="text-xl font-semibold text-gray-900 mb-6">Codex Config</h2>

      {/* Provider form */}
      <div className="bg-white border border-gray-200 rounded-lg p-5 mb-4">
        <h3 className="text-sm font-medium text-gray-900 mb-3">Model Provider</h3>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Name</label>
            <input value={provider.name} onChange={e => setProvider(p => ({ ...p, name: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Base URL</label>
            <input value={provider.base_url} onChange={e => setProvider(p => ({ ...p, base_url: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
        </div>
        <div className="flex items-center gap-4 mt-3">
          <div className="flex-1">
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Wire API</label>
            <select value={provider.wire_api} onChange={e => setProvider(p => ({ ...p, wire_api: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
              <option value="responses">responses</option>
              <option value="chat">chat</option>
            </select>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={provider.requires_openai_auth}
              onChange={e => setProvider(p => ({ ...p, requires_openai_auth: e.target.checked }))} />
            OpenAI Auth
          </label>
        </div>
      </div>

      {/* Profile form */}
      <div className="bg-white border border-gray-200 rounded-lg p-5 mb-4">
        <h3 className="text-sm font-medium text-gray-900 mb-3">Profile</h3>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Name</label>
            <input value={profile.name} onChange={e => setProfile(p => ({ ...p, name: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Model Provider</label>
            <input value={profile.model_provider || ''} onChange={e => setProfile(p => ({ ...p, model_provider: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Model</label>
            <input value={profile.model || ''} onChange={e => setProfile(p => ({ ...p, model: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Reasoning Effort</label>
            <select value={profile.model_reasoning_effort || ''} onChange={e => setProfile(p => ({ ...p, model_reasoning_effort: e.target.value }))}
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
            <select value={profile.preferred_auth_method || ''} onChange={e => setProfile(p => ({ ...p, preferred_auth_method: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
              <option value="">default</option>
              <option value="apikey">apikey</option>
              <option value="oauth">oauth</option>
            </select>
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Approvals Reviewer</label>
            <input value={profile.approvals_reviewer || ''} onChange={e => setProfile(p => ({ ...p, approvals_reviewer: e.target.value }))}
              className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="user" />
          </div>
        </div>
      </div>

      {/* Generated TOML output */}
      <div className="bg-white border border-blue-200 rounded-lg overflow-hidden mb-4">
        <div className="flex items-center justify-between px-4 py-2.5 bg-blue-50 border-b border-blue-100">
          <span className="text-xs font-medium text-blue-700">config.toml 生成结果</span>
          <button
            onClick={() => handleCopy(generatedToml, setCopied)}
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

      {/* auth.json sample */}
      <div className="bg-white border border-green-200 rounded-lg overflow-hidden mb-6">
        <div className="flex items-center justify-between px-4 py-2.5 bg-green-50 border-b border-green-100">
          <span className="text-xs font-medium text-green-700">auth.json 示例</span>
          <button
            onClick={() => handleCopy(AUTH_JSON_SAMPLE, setAuthCopied)}
            className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-green-600 text-white rounded-md hover:bg-green-700 transition-colors"
          >
            {authCopied ? <Check size={12} /> : <Copy size={12} />}
            {authCopied ? '已复制' : '复制'}
          </button>
        </div>
        <pre className="text-xs text-gray-800 p-4 overflow-x-auto whitespace-pre">
          {AUTH_JSON_SAMPLE}
        </pre>
        <div className="px-4 py-2 bg-green-25 border-t border-green-100">
          <p className="text-xs text-green-700">
            PROXY_KEY_ANY_STRING 为任意字符串，代理会将客户端请求中的 key 替换为 config.json 中配置的真实 key。
          </p>
        </div>
      </div>

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
                onClick={() => handleCopy(generateTomlMulti(existingProviders, existingProfiles), setCopied)}
                className="inline-flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
              >
                <Copy size={12} /> 复制
              </button>
            </div>
            <pre className="text-xs text-gray-800 p-4 overflow-x-auto whitespace-pre">
              {generateTomlMulti(existingProviders, existingProfiles) || '# No providers or profiles configured'}
            </pre>
          </div>
        )}
      </div>
    </div>
  )
}