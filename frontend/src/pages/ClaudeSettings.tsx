import { useEffect, useState } from 'react'
import { Copy, Check, ChevronDown, ChevronRight, FileText } from 'lucide-react'
import { useApi } from '../context/ApiContext'

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

export default function ClaudeSettings() {
  const { claudeSettings, loading, fetchClaudeSettings } = useApi()
  const [expanded, setExpanded] = useState<string | null>(null)
  const [copied, setCopied] = useState<string | null>(null)

  useEffect(() => { fetchClaudeSettings() }, [fetchClaudeSettings])

  const handleCopy = async (filename: string, env: Record<string, string>) => {
    const json = generateSettingsJson(env)
    await navigator.clipboard.writeText(json)
    setCopied(filename)
    setTimeout(() => setCopied(null), 2000)
  }

  return (
    <div>
      <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 mb-6">
        <p className="text-sm text-amber-800">
          本页面仅辅助生成配置，你需要自己写配置文件。
        </p>
      </div>

      <h2 className="text-xl font-semibold text-gray-900 mb-6">Claude Settings</h2>

      {loading && <p className="text-sm text-gray-500">Loading...</p>}

      <div className="space-y-2">
        {claudeSettings.map(f => {
          const isExpanded = expanded === f.filename
          const json = generateSettingsJson(f.env)
          return (
            <div key={f.filename} className="bg-white border border-gray-200 rounded-lg overflow-hidden">
              <button
                onClick={() => setExpanded(isExpanded ? null : f.filename)}
                className="w-full flex items-center gap-2 px-4 py-3 hover:bg-gray-50 transition-colors text-left"
              >
                {isExpanded ? <ChevronDown size={14} className="text-gray-400" /> : <ChevronRight size={14} className="text-gray-400" />}
                <FileText size={14} className="text-gray-400" />
                <span className="text-sm font-medium text-gray-900">{f.filename}</span>
                <span className="text-xs text-gray-400 ml-2">
                  {f.env.ANTHROPIC_MODEL && `Model: ${f.env.ANTHROPIC_MODEL}`}
                  {f.env.ANTHROPIC_BASE_URL && ` · ${f.env.ANTHROPIC_BASE_URL}`}
                </span>
              </button>
              {isExpanded && (
                <div className="border-t border-gray-100 px-4 py-3 bg-gray-50">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-xs text-gray-500">settings JSON</span>
                    <button
                      onClick={() => handleCopy(f.filename, f.env)}
                      className="inline-flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
                    >
                      {copied === f.filename ? <Check size={12} className="text-green-500" /> : <Copy size={12} />}
                      {copied === f.filename ? 'Copied' : 'Copy'}
                    </button>
                  </div>
                  <pre className="text-xs text-gray-800 bg-gray-100 rounded-md p-3 overflow-x-auto whitespace-pre">
                    {json}
                  </pre>
                </div>
              )}
            </div>
          )
        })}
      </div>

      {!loading && claudeSettings.length === 0 && (
        <p className="text-sm text-gray-400">No settings files found in ~/.claude/</p>
      )}
    </div>
  )
}
