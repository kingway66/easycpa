import { useEffect, useState } from 'react'
import { Copy, Check, Plus, Eye, FileText } from 'lucide-react'
import { useApi } from '../context/api-context'

const ENV_FIELDS = [
  'ANTHROPIC_BASE_URL',
  'ANTHROPIC_AUTH_TOKEN',
  'ANTHROPIC_MODEL',
  'ANTHROPIC_DEFAULT_OPUS_MODEL',
  'ANTHROPIC_DEFAULT_SONNET_MODEL',
  'ANTHROPIC_DEFAULT_HAIKU_MODEL',
  'CLAUDE_CODE_SUBAGENT_MODEL',
  'CLAUDE_CODE_EFFORT_LEVEL',
]

function generateSettingsJson(env: Record<string, string>) {
  const obj: Record<string, string> = {}
  ENV_FIELDS.forEach(f => { if (env[f]) obj[f] = env[f] })
  return JSON.stringify({ env: obj }, null, 2)
}

const CCS_PROXY_PRESET: Record<string, string> = {
  ANTHROPIC_BASE_URL: 'http://127.0.0.1:15791',
  ANTHROPIC_AUTH_TOKEN: 'PROXY_KEY_ANY_STRING',
  ANTHROPIC_MODEL: 'deepseek-v4-pro',
  ANTHROPIC_DEFAULT_OPUS_MODEL: 'deepseek-v4-pro',
  ANTHROPIC_DEFAULT_SONNET_MODEL: 'deepseek-v4-pro',
  ANTHROPIC_DEFAULT_HAIKU_MODEL: 'deepseek-v4-flash',
  CLAUDE_CODE_SUBAGENT_MODEL: 'deepseek-v4-flash',
  CLAUDE_CODE_EFFORT_LEVEL: 'high',
}

export default function ClaudeSettings() {
  const { claudeSettings, loading, fetchClaudeSettings } = useApi()
  const [form, setForm] = useState<Record<string, string>>(CCS_PROXY_PRESET)
  const [copied, setCopied] = useState(false)
  const [viewing, setViewing] = useState<string | null>(null)

  useEffect(() => { fetchClaudeSettings() }, [fetchClaudeSettings])

  const update = (key: string, value: string) => {
    setForm(f => ({ ...f, [key]: value }))
  }

  const generatedJson = generateSettingsJson(form)

  const handleCopy = async (text: string) => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const handleViewCopy = async (env: Record<string, string>) => {
    await navigator.clipboard.writeText(generateSettingsJson(env))
    setViewing(null)
  }

  return (
    <div>
      <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 mb-6">
        <p className="text-sm text-amber-800">
          本页面仅辅助生成配置，你需要自己写配置文件。
        </p>
        <p className="text-sm text-amber-700 mt-1">
          例如写入 <code className="bg-amber-100 px-1 rounded">~/.claude/settings.easycpa.json</code>，然后用 <code className="bg-amber-100 px-1 rounded">claude --settings ~/.claude/settings.easycpa.json</code> 启动。
        </p>
      </div>

      <h2 className="text-xl font-semibold text-gray-900 mb-6">Claude Settings</h2>

      {/* Generator form */}
      <div className="bg-white border border-gray-200 rounded-lg p-5 mb-6">
        <div className="flex items-center gap-2 mb-4">
          <Plus size={16} className="text-blue-600" />
          <h3 className="text-sm font-medium text-gray-900">生成新的 settings 文件</h3>
        </div>
        <div className="grid grid-cols-2 gap-3">
          {ENV_FIELDS.map(field => (
            <div key={field}>
              <label className="block text-xs font-medium text-gray-600 mb-0.5">{field}</label>
              <input
                type={field.includes('TOKEN') ? 'text' : 'text'}
                value={form[field] || ''}
                onChange={e => update(field, e.target.value)}
                placeholder={field.includes('URL') ? 'https://...' : field.includes('TOKEN') ? 'sk-...' : ''}
                className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
          ))}
        </div>
        {Object.values(form).some(v => v) && (
          <div className="mt-4">
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs font-medium text-gray-600">生成结果</span>
              <button
                onClick={() => handleCopy(generatedJson)}
                className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
              >
                {copied ? <Check size={12} /> : <Copy size={12} />}
                {copied ? '已复制' : '复制'}
              </button>
            </div>
            <pre className="text-xs text-gray-800 bg-gray-50 border border-gray-200 rounded-md p-3 overflow-x-auto whitespace-pre">
              {generatedJson}
            </pre>
          </div>
        )}
      </div>

      {/* Existing files — read only */}
      <div className="mb-4">
        <div className="flex items-center gap-2 mb-3">
          <Eye size={14} className="text-gray-400" />
          <h3 className="text-sm font-medium text-gray-700">已有 settings 文件（仅供参考）</h3>
        </div>

        {loading && <p className="text-sm text-gray-500">Loading...</p>}

        <div className="space-y-2">
          {claudeSettings.map(f => {
            const isViewing = viewing === f.filename
            return (
              <div key={f.filename} className="bg-white border border-gray-200 rounded-lg overflow-hidden">
                <button
                  onClick={() => setViewing(isViewing ? null : f.filename)}
                  className="w-full flex items-center gap-2 px-4 py-3 hover:bg-gray-50 transition-colors text-left"
                >
                  <FileText size={14} className="text-gray-400" />
                  <span className="text-sm font-medium text-gray-900">{f.filename}</span>
                  <span className="text-xs text-gray-400 ml-2">
                    {f.env.ANTHROPIC_MODEL && `Model: ${f.env.ANTHROPIC_MODEL}`}
                    {f.env.ANTHROPIC_BASE_URL && ` · ${f.env.ANTHROPIC_BASE_URL}`}
                  </span>
                </button>
                {isViewing && (
                  <div className="border-t border-gray-100 px-4 py-3 bg-gray-50">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-xs text-gray-500">{f.filename} 内容</span>
                      <button
                        onClick={() => handleViewCopy(f.env)}
                        className="inline-flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
                      >
                        <Copy size={12} /> 复制
                      </button>
                    </div>
                    <pre className="text-xs text-gray-800 bg-gray-100 rounded-md p-3 overflow-x-auto whitespace-pre">
                      {generateSettingsJson(f.env)}
                    </pre>
                  </div>
                )}
              </div>
            )
          })}
        </div>

        {!loading && claudeSettings.length === 0 && (
          <p className="text-sm text-gray-400 mt-2">~/.claude/ 中暂无 settings 文件</p>
        )}
      </div>
    </div>
  )
}
