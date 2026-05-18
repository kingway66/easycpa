import { useEffect, useState } from 'react'
import { Save, Trash2, Plus, FileText } from 'lucide-react'
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

export default function ClaudeSettings() {
  const { claudeSettings, loading, fetchClaudeSettings, saveClaudeSettings, deleteClaudeSettings } = useApi()
  const [editing, setEditing] = useState<string | null>(null)
  const [form, setForm] = useState<Record<string, string>>({})
  const [saving, setSaving] = useState(false)
  const [showNew, setShowNew] = useState(false)
  const [newName, setNewName] = useState('')

  useEffect(() => { fetchClaudeSettings() }, [fetchClaudeSettings])

  const startEdit = (filename: string, env: Record<string, string>) => {
    const initial: Record<string, string> = {}
    ENV_FIELDS.forEach(f => { initial[f] = env[f] || '' })
    setForm(initial)
    setEditing(filename)
    setShowNew(false)
  }

  const startNew = () => {
    const initial: Record<string, string> = {}
    ENV_FIELDS.forEach(f => { initial[f] = '' })
    setForm(initial)
    setNewName('')
    setEditing(null)
    setShowNew(true)
  }

  const handleSave = async (filename: string) => {
    setSaving(true)
    try {
      const env: Record<string, string> = {}
      Object.entries(form).forEach(([k, v]) => { if (v) env[k] = v })
      await saveClaudeSettings(filename, env)
      setEditing(null)
      setShowNew(false)
    } catch (e) {
      alert(`Save failed: ${e}`)
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (filename: string) => {
    if (!confirm(`Delete ${filename}?`)) return
    await deleteClaudeSettings(filename)
    setEditing(null)
  }

  const update = (key: string, value: string) => {
    setForm(f => ({ ...f, [key]: value }))
  }

  const editForm = (filename: string) => (
    <div className="bg-white border border-gray-200 rounded-lg p-5 space-y-3">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-medium text-gray-900">{filename}</h3>
        <div className="flex gap-2">
          <button onClick={() => handleSave(filename)} disabled={saving}
            className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50">
            <Save size={12} /> {saving ? 'Saving...' : 'Save'}
          </button>
          <button onClick={() => handleDelete(filename)}
            className="inline-flex items-center gap-1 px-3 py-1.5 text-xs text-red-600 border border-red-300 rounded-md hover:bg-red-50">
            <Trash2 size={12} /> Delete
          </button>
          <button onClick={() => setEditing(null)}
            className="px-3 py-1.5 text-xs text-gray-500 border border-gray-200 rounded-md hover:bg-gray-50">Cancel</button>
        </div>
      </div>
      {ENV_FIELDS.map(field => (
        <div key={field}>
          <label className="block text-xs font-medium text-gray-600 mb-0.5">{field}</label>
          <input type={field.includes('TOKEN') || field.includes('KEY') ? 'password' : 'text'}
            value={form[field] || ''} onChange={e => update(field, e.target.value)}
            className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
      ))}
    </div>
  )

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-xl font-semibold text-gray-900">Claude Settings</h2>
        <button onClick={startNew}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700">
          <Plus size={14} /> New Settings File
        </button>
      </div>

      {loading && <p className="text-sm text-gray-500">Loading...</p>}

      {/* New file form */}
      {showNew && (
        <div className="bg-white border border-blue-300 rounded-lg p-5 space-y-3 mb-4">
          <div>
            <label className="block text-xs font-medium text-gray-600 mb-0.5">Filename</label>
            <div className="flex items-center gap-1">
              <span className="text-sm text-gray-500">settings.</span>
              <input type="text" value={newName}
                onChange={e => setNewName(e.target.value)}
                placeholder="e.g. mymodel"
                className="px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <span className="text-sm text-gray-500">.json</span>
            </div>
          </div>
          {ENV_FIELDS.map(field => (
            <div key={field}>
              <label className="block text-xs font-medium text-gray-600 mb-0.5">{field}</label>
              <input type={field.includes('TOKEN') || field.includes('KEY') ? 'password' : 'text'}
                value={form[field] || ''} onChange={e => update(field, e.target.value)}
                className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
          ))}
          <div className="flex gap-2 pt-1">
            <button onClick={() => newName && handleSave(`settings.${newName}.json`)} disabled={saving || !newName}
              className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50">
              <Save size={12} /> {saving ? 'Creating...' : 'Create'}
            </button>
            <button onClick={() => setShowNew(false)}
              className="px-3 py-1.5 text-xs text-gray-500 border border-gray-200 rounded-md hover:bg-gray-50">Cancel</button>
          </div>
        </div>
      )}

      {/* Existing files */}
      <div className="space-y-3">
        {claudeSettings.map(f => (
          editing === f.filename ? (
            <div key={f.filename}>{editForm(f.filename)}</div>
          ) : (
            <div key={f.filename}
              onClick={() => startEdit(f.filename, f.env)}
              className="p-4 bg-white border border-gray-200 rounded-lg hover:border-blue-400 cursor-pointer transition-colors">
              <div className="flex items-center gap-2 mb-1">
                <FileText size={14} className="text-gray-400" />
                <span className="text-sm font-medium text-gray-900">{f.filename}</span>
              </div>
              <div className="text-xs text-gray-500">
                {f.env.ANTHROPIC_BASE_URL && <div>URL: {f.env.ANTHROPIC_BASE_URL}</div>}
                {f.env.ANTHROPIC_MODEL && <div>Model: {f.env.ANTHROPIC_MODEL}</div>}
              </div>
            </div>
          )
        ))}
      </div>
    </div>
  )
}
