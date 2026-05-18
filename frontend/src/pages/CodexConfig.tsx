import { useEffect, useState } from 'react'
import { Save, Trash2, Plus } from 'lucide-react'
import { useApi } from '../context/ApiContext'
import type { CodexModelProvider, CodexProfile } from '../context/ApiContext'

export default function CodexConfig() {
  const { codexConfig, loading, fetchCodexConfig, saveCodexModelProvider, deleteCodexModelProvider, saveCodexProfile, deleteCodexProfile } = useApi()
  const [editProvider, setEditProvider] = useState<CodexModelProvider | null>(null)
  const [editProfile, setEditProfile] = useState<CodexProfile | null>(null)
  const [newProvider, setNewProvider] = useState(false)
  const [newProfile, setNewProfile] = useState(false)
  const [saving, setSaving] = useState(false)

  const [pForm, setPForm] = useState<CodexModelProvider>({ name: '', base_url: '', wire_api: 'responses', requires_openai_auth: true })
  const [prForm, setPrForm] = useState<CodexProfile>({ name: '', model_provider: '', model: '', model_reasoning_effort: 'high', preferred_auth_method: 'apikey' })

  useEffect(() => { fetchCodexConfig() }, [fetchCodexConfig])

  const handleSaveProvider = async () => {
    setSaving(true)
    try {
      await saveCodexModelProvider(pForm)
      setEditProvider(null)
      setNewProvider(false)
    } catch (e) { alert(`Save failed: ${e}`) }
    finally { setSaving(false) }
  }

  const handleSaveProfile = async () => {
    setSaving(true)
    try {
      await saveCodexProfile(prForm)
      setEditProfile(null)
      setNewProfile(false)
    } catch (e) { alert(`Save failed: ${e}`) }
    finally { setSaving(false) }
  }

  const handleDeleteProvider = async (name: string) => {
    if (!confirm(`Delete provider "${name}"?`)) return
    await deleteCodexModelProvider(name)
  }

  const handleDeleteProfile = async (name: string) => {
    if (!confirm(`Delete profile "${name}"?`)) return
    await deleteCodexProfile(name)
  }

  const startEditProvider = (p: CodexModelProvider) => {
    setPForm({ ...p })
    setEditProvider(p)
    setNewProvider(false)
  }

  const startEditProfile = (p: CodexProfile) => {
    setPrForm({ ...p })
    setEditProfile(p)
    setNewProfile(false)
  }

  const providers = codexConfig?.model_providers || []
  const profiles = codexConfig?.profiles || []

  return (
    <div>
      <h2 className="text-xl font-semibold text-gray-900 mb-6">Codex Config</h2>
      {loading && <p className="text-sm text-gray-500">Loading...</p>}

      {/* Model Providers */}
      <div className="mb-8">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-medium text-gray-700">Model Providers</h3>
          <button onClick={() => { setPForm({ name: '', base_url: '', wire_api: 'responses', requires_openai_auth: true }); setNewProvider(true); setEditProvider(null) }}
            className="inline-flex items-center gap-1 px-2.5 py-1 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700">
            <Plus size={12} /> Add Provider
          </button>
        </div>

        <div className="space-y-2">
          {providers.map(p => (
            editProvider?.name === p.name || (newProvider && false) ? null : (
              <div key={p.name}
                onClick={() => startEditProvider(p)}
                className="p-3 bg-white border border-gray-200 rounded-lg hover:border-blue-400 cursor-pointer transition-colors">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-gray-900">{p.name}</span>
                  <button onClick={e => { e.stopPropagation(); handleDeleteProvider(p.name) }}
                    className="text-red-400 hover:text-red-600"><Trash2 size={14} /></button>
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  {p.base_url} · {p.wire_api}
                </div>
              </div>
            )
          ))}
        </div>

        {/* Provider edit form */}
        {(editProvider || newProvider) && (
          <div className="mt-2 bg-white border border-blue-300 rounded-lg p-4 space-y-3">
            <h4 className="text-xs font-medium text-gray-600">{newProvider ? 'New Provider' : `Edit: ${pForm.name}`}</h4>
            {newProvider && (
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Name</label>
                <input value={pForm.name} onChange={e => setPForm((f: CodexModelProvider) => ({ ...f, name: e.target.value }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
            )}
            <div>
              <label className="block text-xs font-medium text-gray-600 mb-0.5">Base URL</label>
              <input value={pForm.base_url} onChange={e => setPForm((f: CodexModelProvider) => ({ ...f, base_url: e.target.value }))}
                className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
            </div>
            <div>
              <label className="block text-xs font-medium text-gray-600 mb-0.5">Wire API</label>
              <select value={pForm.wire_api} onChange={e => setPForm((f: CodexModelProvider) => ({ ...f, wire_api: e.target.value }))}
                className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
                <option value="responses">responses</option>
                <option value="chat">chat</option>
              </select>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={pForm.requires_openai_auth}
                onChange={e => setPForm((f: CodexModelProvider) => ({ ...f, requires_openai_auth: e.target.checked }))} />
              Requires OpenAI Auth
            </label>
            <div className="flex gap-2">
              <button onClick={handleSaveProvider} disabled={saving}
                className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50">
                <Save size={12} /> {saving ? 'Saving...' : 'Save'}
              </button>
              <button onClick={() => { setEditProvider(null); setNewProvider(false) }}
                className="px-3 py-1.5 text-xs text-gray-500 border border-gray-200 rounded-md hover:bg-gray-50">Cancel</button>
            </div>
          </div>
        )}
      </div>

      {/* Profiles */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-medium text-gray-700">Profiles</h3>
          <button onClick={() => { setPrForm({ name: '', model_provider: '', model: '', model_reasoning_effort: 'high', preferred_auth_method: 'apikey' }); setNewProfile(true); setEditProfile(null) }}
            className="inline-flex items-center gap-1 px-2.5 py-1 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700">
            <Plus size={12} /> Add Profile
          </button>
        </div>

        <div className="space-y-2">
          {profiles.map(p => (
            editProfile?.name === p.name ? null : (
              <div key={p.name}
                onClick={() => startEditProfile(p)}
                className="p-3 bg-white border border-gray-200 rounded-lg hover:border-blue-400 cursor-pointer transition-colors">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-gray-900">{p.name}</span>
                  <button onClick={e => { e.stopPropagation(); handleDeleteProfile(p.name) }}
                    className="text-red-400 hover:text-red-600"><Trash2 size={14} /></button>
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  {p.model_provider && <span>{p.model_provider} · </span>}
                  {p.model && <span>{p.model} · </span>}
                  {p.model_reasoning_effort}
                </div>
              </div>
            )
          ))}
        </div>

        {/* Profile edit form */}
        {(editProfile || newProfile) && (
          <div className="mt-2 bg-white border border-blue-300 rounded-lg p-4 space-y-3">
            <h4 className="text-xs font-medium text-gray-600">{newProfile ? 'New Profile' : `Edit: ${prForm.name}`}</h4>
            {newProfile && (
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Name</label>
                <input value={prForm.name} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, name: e.target.value }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
            )}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Model Provider</label>
                <input value={prForm.model_provider || ''} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, model_provider: e.target.value }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Model</label>
                <input value={prForm.model || ''} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, model: e.target.value }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Reasoning Effort</label>
                <select value={prForm.model_reasoning_effort || ''} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, model_reasoning_effort: e.target.value }))}
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
                <select value={prForm.preferred_auth_method || ''} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, preferred_auth_method: e.target.value }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
                  <option value="">default</option>
                  <option value="apikey">apikey</option>
                  <option value="oauth">oauth</option>
                </select>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Context Window</label>
                <input type="number" value={prForm.model_context_window || ''} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, model_context_window: e.target.value ? parseInt(e.target.value) : undefined }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-0.5">Auto Compact Limit</label>
                <input type="number" value={prForm.model_auto_compact_token_limit || ''} onChange={e => setPrForm((f: CodexProfile) => ({ ...f, model_auto_compact_token_limit: e.target.value ? parseInt(e.target.value) : undefined }))}
                  className="w-full px-3 py-1.5 text-sm border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500" />
              </div>
            </div>
            <div className="flex gap-2 pt-1">
              <button onClick={handleSaveProfile} disabled={saving}
                className="inline-flex items-center gap-1 px-3 py-1.5 text-xs bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50">
                <Save size={12} /> {saving ? 'Saving...' : 'Save'}
              </button>
              <button onClick={() => { setEditProfile(null); setNewProfile(false) }}
                className="px-3 py-1.5 text-xs text-gray-500 border border-gray-200 rounded-md hover:bg-gray-50">Cancel</button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
