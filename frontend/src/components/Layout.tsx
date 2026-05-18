import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { Settings, Cpu, Plus, X } from 'lucide-react'
import { useApi } from '../context/ApiContext'
import MODEL_TEMPLATES from '../lib/templates'

export default function Layout() {
  const { models, deleteModel, configPath } = useApi()
  const location = useLocation()
  const navigate = useNavigate()

  // Ensure models are loaded for sidebar counts
  const modelNames = new Set(models.map(m => m.name))

  return (
    <div className="flex h-screen bg-gray-50">
      <aside className="w-60 bg-white border-r border-gray-200 flex flex-col">
        <div className="px-5 py-4 border-b border-gray-200">
          <h1 className="text-lg font-semibold text-gray-900">EasyCPA</h1>
          <p className="text-xs text-gray-500 mt-0.5">Configuration Manager</p>
          {configPath && <p className="text-[10px] text-gray-400 mt-1 truncate" title={configPath}>Config: {configPath}</p>}
        </div>
        <nav className="flex-1 overflow-y-auto px-3 py-3">
          {/* Config sections */}
          <div className="space-y-1 mb-4">
            <NavLink
              to="/claude-settings"
              className={({ isActive }) =>
                `flex items-center gap-2.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive ? 'bg-blue-50 text-blue-700' : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
                }`
              }
            >
              <Settings size={16} />
              Claude Settings
            </NavLink>
            <NavLink
              to="/codex-config"
              className={({ isActive }) =>
                `flex items-center gap-2.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive ? 'bg-blue-50 text-blue-700' : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
                }`
              }
            >
              <Cpu size={16} />
              Codex Config
            </NavLink>
          </div>

          {/* Divider */}
          <div className="border-t border-gray-200 my-2" />

          {/* Model list */}
          <div className="mt-2">
            <div className="flex items-center justify-between px-3 mb-1">
              <p className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider">Models</p>
              <button
                onClick={() => {
                  const name = prompt('Model name:')
                  if (name?.trim()) navigate(`/models/${encodeURIComponent(name.trim())}`)
                }}
                className="text-gray-400 hover:text-blue-600"
                title="Add model"
              >
                <Plus size={12} />
              </button>
            </div>
            <div className="space-y-0.5">
              {MODEL_TEMPLATES.map(t => {
                const configured = modelNames.has(t.name)
                const isActive = location.pathname === `/models/${encodeURIComponent(t.name)}`
                return (
                  <div key={t.name} className="group relative">
                    <NavLink
                      to={`/models/${encodeURIComponent(t.name)}`}
                      className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-sm transition-colors ${
                        isActive
                          ? 'bg-blue-50 text-blue-700 font-medium'
                          : configured
                            ? 'text-gray-700 hover:bg-gray-100'
                            : 'text-gray-400 hover:bg-gray-50 hover:text-gray-600'
                      }`}
                    >
                      <span className="text-sm">{t.icon}</span>
                      <span>{t.label}</span>
                      {configured && !isActive && <span className="ml-auto w-1.5 h-1.5 rounded-full bg-blue-400 group-hover:hidden" />}
                    </NavLink>
                    {configured && (
                      <button
                        onClick={async (e) => {
                          e.preventDefault()
                          if (!confirm(`Delete model "${t.name}"? All routes will be removed.`)) return
                          await deleteModel(t.name)
                          if (isActive) navigate('/claude-settings')
                        }}
                        className="absolute right-2 top-1/2 -translate-y-1/2 hidden group-hover:flex items-center justify-center w-4 h-4 rounded text-gray-400 hover:text-red-500"
                      >
                        <X size={12} />
                      </button>
                    )}
                  </div>
                )
              })}
              {/* Custom (non-template) models */}
              {// Deduplicate by name for sidebar display
                [...new Map(models.map(m => [m.name, m])).values()]
                  .filter(m => !MODEL_TEMPLATES.some(t => t.name === m.name))
                  .map(m => {
                  const isActive = location.pathname === `/models/${encodeURIComponent(m.name)}`
                  return (
                    <div key={m.name} className="group relative">
                      <NavLink
                        to={`/models/${encodeURIComponent(m.name)}`}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-sm transition-colors ${
                          isActive
                            ? 'bg-blue-50 text-blue-700 font-medium'
                            : 'text-gray-700 hover:bg-gray-100'
                        }`}
                      >
                        <span className="text-sm">⚪</span>
                        <span>{m.name}</span>
                        {!isActive && <span className="ml-auto w-1.5 h-1.5 rounded-full bg-blue-400 group-hover:hidden" />}
                      </NavLink>
                      <button
                        onClick={async (e) => {
                          e.preventDefault()
                          if (!confirm(`Delete model "${m.name}"? All routes will be removed.`)) return
                          await deleteModel(m.name)
                          if (isActive) navigate('/claude-settings')
                        }}
                        className="absolute right-2 top-1/2 -translate-y-1/2 hidden group-hover:flex items-center justify-center w-4 h-4 rounded text-gray-400 hover:text-red-500"
                      >
                        <X size={12} />
                      </button>
                    </div>
                  )
                })}
            </div>
          </div>
        </nav>
      </aside>
      <main className="flex-1 overflow-auto">
        <div className="max-w-4xl mx-auto px-6 py-6">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
