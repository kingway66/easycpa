import { useEffect, useState } from 'react'
import { useParams } from 'react-router-dom'
import { Save, Trash2, Plus } from 'lucide-react'
import { useApi } from '../context/ApiContext'

import MODEL_TEMPLATES from '../lib/templates'

interface ModelForm {
  name: string
  model: string
  base_url: string
  api_key: string
  proxy_url: string
  api_format: string
  context_window: string
  max_output_tokens: string
  default_reasoning_level: string
  supported_reasoning_levels: string
  supports_parallel_tool_calls: boolean
  supports_reasoning_summaries: boolean
  enabled: boolean
}

const EMPTY_FORM: ModelForm = {
  name: '', model: '', base_url: '', api_key: '', proxy_url: '', api_format: 'openai_chat',
  context_window: '', max_output_tokens: '', default_reasoning_level: '',
  supported_reasoning_levels: '', supports_parallel_tool_calls: false, supports_reasoning_summaries: false, enabled: false,
}

const template = (name: string) => MODEL_TEMPLATES.find(t => t.name === name)

export default function ModelDetail() {
  const { name } = useParams<{ name: string }>()
  const { models, loading, fetchModels, saveModel, deleteModel } = useApi()
  const [editingKey, setEditingKey] = useState<string | null>(null)
  const [form, setForm] = useState<ModelForm>(EMPTY_FORM)
  const [saving, setSaving] = useState(false)
  const [savingRadio, setSavingRadio] = useState<string | null>(null)

  useEffect(() => { fetchModels() }, [fetchModels])

  // Routes matching this name (could be multiple with same name)
  const matchingRoutes = models.filter(m => m.name === name)
  const tmpl = template(name!)
  const icon = tmpl?.icon || '⚪'
  const label = tmpl?.label || name!

  const cardKey = (m: { base_url: string; model: string }) => m.base_url + m.model

  const startAdd = () => {
    const t = template(name!)
    setForm({
      ...EMPTY_FORM,
      name: name!,
      model: t?.model || name!,
      api_format: t?.api_format || 'openai_chat',
    })
    setEditingKey('__new__')
  }

  const startEdit = (m: typeof models[0]) => {
    setForm({
      name: m.name,
      model: m.model,
      base_url: m.base_url,
      api_key: m.api_key,
      proxy_url: m.proxy_url || '',
      api_format: m.api_format,
      context_window: m.context_window?.toString() || '',
      max_output_tokens: m.max_output_tokens?.toString() || '',
      default_reasoning_level: m.default_reasoning_level || '',
      supported_reasoning_levels: m.supported_reasoning_levels?.join(', ') || '',
      supports_parallel_tool_calls: m.supports_parallel_tool_calls ?? false,
      supports_reasoning_summaries: m.supports_reasoning_summaries ?? false,
      enabled: m.enabled ?? false,
    })
    setEditingKey(cardKey(m))
  }

  const handleSave = async () => {
    setSaving(true)
    try {
      await saveModel({
        name: form.name,
        model: form.model || form.name,
        base_url: form.base_url,
        api_key: form.api_key,
        proxy_url: form.proxy_url || undefined,
        api_format: form.api_format,
        context_window: form.context_window ? parseInt(form.context_window) : undefined,
        max_output_tokens: form.max_output_tokens ? parseInt(form.max_output_tokens) : undefined,
        default_reasoning_level: form.default_reasoning_level || undefined,
        supported_reasoning_levels: form.supported_reasoning_levels
          ? form.supported_reasoning_levels.split(',').map(s => s.trim()).filter(Boolean)
          : undefined,
        supports_parallel_tool_calls: form.supports_parallel_tool_calls || undefined,
        supports_reasoning_summaries: form.supports_reasoning_summaries || undefined,
        enabled: form.enabled,
      })
      setEditingKey(null)
    } catch (e) {
      alert(`Save failed: ${e}`)
    } finally {
      setSaving(false)
    }
  }

  const handleRadioSelect = async (m: typeof models[0]) => {
    setSavingRadio(cardKey(m))
    try {
      await saveModel({ ...m, enabled: true })
    } catch (e) {
      alert(`Failed: ${e}`)
    } finally {
      setSavingRadio(null)
    }
  }

  const handleDelete = async (m: typeof models[0]) => {
    if (!confirm(`Delete route "${m.model}"?`)) return
    await deleteModel(m.name, m.base_url, m.model)
  }

  const update = (key: keyof ModelForm, value: string | boolean) => {
    setForm(f => ({ ...f, [key]: value }))
  }

  const textField = (label: string, key: keyof ModelForm, placeholder?: string, type = 'text', readOnly = false) => (
    <div>
      <label className="block text-xs font-medium text-gray-600 mb-0.5">{label}</label>
      <input
        type={type}
        value={form[key] as string}
        onChange={e => update(key, e.target.value)}
        placeholder={placeholder}
        readOnly={readOnly}
        className={`w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 ${readOnly ? 'bg-gray-100 cursor-not-allowed' : ''}`}
      />
    </div>
  )

  const editForm = () => (
    <div className="bg-white border border-blue-300 rounded-lg p-5 space-y-3">
      <div className="grid grid-cols-2 gap-4">
        {textField('Route Name', 'name', 'e.g. gpt-5.5', 'text', true)}
        {textField('Model (upstream)', 'model', 'actual model name sent to upstream')}
      </div>
      <div className="grid grid-cols-2 gap-4">
        {textField('Base URL', 'base_url', 'https://api.example.com/v1')}
        {textField('API Key', 'api_key', 'sk-...')}
      </div>

      <div className="grid grid-cols-2 gap-4">
        {textField('Proxy URL', 'proxy_url', 'http://127.0.0.1:7890 or socks5://...')}
      </div>

      <div>
        <label className="block text-xs font-medium text-gray-600 mb-0.5">API Format</label>
        <select
          value={form.api_format}
          onChange={e => update('api_format', e.target.value)}
          className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          <option value="openai_chat">OpenAI Chat</option>
          <option value="openai_responses">OpenAI Responses</option>
          <option value="anthropic">Anthropic</option>
        </select>
      </div>

      <div className="grid grid-cols-2 gap-4">
        {textField('Context Window', 'context_window', '1000000')}
        {textField('Max Output Tokens', 'max_output_tokens', '384000')}
      </div>

      <div className="grid grid-cols-2 gap-4">
        {textField('Default Reasoning Level', 'default_reasoning_level', 'high')}
        {textField('Supported Reasoning Levels', 'supported_reasoning_levels', 'low, medium, high, xhigh')}
      </div>

      <div className="flex gap-6">
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.enabled}
            onChange={e => update('enabled', e.target.checked)} />
          Enabled
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.supports_parallel_tool_calls}
            onChange={e => update('supports_parallel_tool_calls', e.target.checked)} />
          Parallel Tool Calls
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.supports_reasoning_summaries}
            onChange={e => update('supports_reasoning_summaries', e.target.checked)} />
          Reasoning Summaries
        </label>
      </div>

      <div className="flex items-center gap-3 pt-2">
        <button
          onClick={handleSave}
          disabled={saving || !form.name}
          className="inline-flex items-center gap-1.5 px-4 py-2 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 transition-colors"
        >
          <Save size={14} /> {saving ? 'Saving...' : 'Save'}
        </button>
        <button
          onClick={() => setEditingKey(null)}
          className="px-4 py-2 text-sm text-gray-500 border border-gray-200 rounded-md hover:bg-gray-50"
        >
          Cancel
        </button>
      </div>
    </div>
  )

  return (
    <div>
      <div className="flex items-center gap-3 mb-6">
        <span className="text-2xl">{icon}</span>
        <h2 className="text-xl font-semibold text-gray-900">{label}</h2>
        {matchingRoutes.length > 0 && (
          <span className="text-xs text-gray-400">{matchingRoutes.length} route{matchingRoutes.length > 1 ? 's' : ''}</span>
        )}
      </div>

      {loading && <p className="text-sm text-gray-500">Loading...</p>}

      {/* Existing routes */}
      {matchingRoutes.length > 0 && (
        <div className="space-y-3 mb-4">
          {matchingRoutes.map(m => (
            <div key={cardKey(m)}>
              <div
                onClick={() => startEdit(m)}
                className={`p-4 bg-white border rounded-lg hover:border-blue-400 cursor-pointer transition-colors ${m.enabled === false ? 'border-gray-300 opacity-60' : 'border-gray-200'}`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <input
                      type="radio"
                      name={`route-${name}`}
                      checked={m.enabled !== false}
                      disabled={savingRadio === cardKey(m)}
                      onClick={e => { e.stopPropagation(); handleRadioSelect(m) }}
                      className="w-3.5 h-3.5 text-blue-600"
                    />
                    <span className={`text-sm font-medium ${m.enabled === false ? 'text-gray-400' : 'text-gray-900'}`}>{m.model}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    {m.enabled === false && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-red-50 text-red-500 font-medium">Disabled</span>
                    )}
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 text-gray-600 font-mono">{m.api_format}</span>
                    <button
                      onClick={e => { e.stopPropagation(); handleDelete(m) }}
                      className="text-red-400 hover:text-red-600"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
                <div className="text-xs text-gray-500 space-y-0.5">
                  <div>URL: {m.base_url || 'not configured'}</div>
                  {m.proxy_url && <div>Proxy: {m.proxy_url}</div>}
                  {m.context_window && <div>Context: {m.context_window.toLocaleString()}</div>}
                  {m.default_reasoning_level && <div>Reasoning: {m.default_reasoning_level}</div>}
                </div>
              </div>
              {/* Edit form below this card */}
              {editingKey === cardKey(m) && <div className="mt-3">{editForm()}</div>}
            </div>
          ))}
        </div>
      )}

      {/* No routes configured */}
      {matchingRoutes.length === 0 && editingKey !== '__new__' && (
        <div className="text-center py-12">
          <p className="text-gray-400 mb-4">尚未配置 {label}</p>
          <button
            onClick={startAdd}
            className="inline-flex items-center gap-1.5 px-4 py-2 text-sm bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
          >
            <Plus size={16} /> 添加配置
          </button>
        </div>
      )}

      {/* Add form (at bottom, for new routes) */}
      {editingKey === '__new__' && (
        <div className="mb-4">{editForm()}</div>
      )}

      {/* Add another route button (when routes exist but not editing) */}
      {matchingRoutes.length > 0 && editingKey === null && (
        <button
          onClick={startAdd}
          className="inline-flex items-center gap-1.5 text-sm text-blue-600 hover:text-blue-700"
        >
          <Plus size={14} /> 添加配置
        </button>
      )}
    </div>
  )
}
