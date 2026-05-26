import { useEffect, useState } from 'react'
import { useApi } from '../context/api-context'
import { Activity, RefreshCw, RotateCw, Clock, FileText, Server, Hash } from 'lucide-react'

interface StatusData {
  running: boolean
  address: string
  port: number
  uptime_seconds: number
  version: string
  pid: number
  started_at: string
  config_path: string
  active_connections: number
  total_requests: number
  success_requests: number
  failed_requests: number
  success_rate: number
  active_targets: { app_type: string; provider_name: string; provider_id: string }[]
}

function formatUptime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return `${h}h ${m}m`
}

function formatTime(iso: string): string {
  if (!iso) return '-'
  try {
    return new Date(iso).toLocaleString()
  } catch {
    return iso
  }
}

export default function Dashboard() {
  const { models, fetchModels, serverReload, serverRestart } = useApi()
  const [status, setStatus] = useState<StatusData | null>(null)
  const [reloading, setReloading] = useState(false)
  const [restarting, setRestarting] = useState(false)

  async function fetchStatus() {
    try {
      const res = await fetch('/status')
      if (res.ok) {
        const data = await res.json()
        setStatus(data)
      }
    } catch { /* ignore */ }
  }

  useEffect(() => {
    void fetch('/status')
      .then(async (res) => {
        if (!res.ok) return null
        return res.json()
      })
      .then((data) => {
        if (data) setStatus(data)
      })
      .catch(() => {})

    void fetchModels()
  }, [fetchModels])

  // Auto-refresh every 5s
  useEffect(() => {
    const timer = setInterval(() => {
      void fetchStatus()
    }, 5000)
    return () => clearInterval(timer)
  }, [])

  const handleReload = async () => {
    if (!confirm('Reload EasyCPA configuration?')) return
    setReloading(true)
    try {
      await serverReload()
      await fetchStatus()
    } finally {
      setReloading(false)
    }
  }

  const handleRestart = async () => {
    if (!confirm('Restart EasyCPA? Current connections will be interrupted.')) return
    setRestarting(true)
    try {
      await serverRestart()
      // Poll /health until back
      let tries = 0
      while (tries < 30) {
        await new Promise(r => setTimeout(r, 1000))
        try {
          const res = await fetch('/health')
          if (res.ok) break
        } catch { /* not back yet */ }
        tries++
      }
      await fetchStatus()
    } finally {
      setRestarting(false)
    }
  }

  const enabledModels = models.filter(m => m.enabled !== false)
  const totalRoutes = models.length

  return (
    <div className="space-y-6">
      {/* Status Card */}
      <div className="bg-white rounded-lg border border-gray-200 p-5">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-base font-semibold text-gray-900 flex items-center gap-2">
            <Activity size={18} />
            Service Status
          </h2>
          <span className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium ${
            status?.running ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'
          }`}>
            <span className={`w-1.5 h-1.5 rounded-full ${status?.running ? 'bg-green-500' : 'bg-red-500'}`} />
            {status?.running ? 'Running' : 'Stopped'}
          </span>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          <div className="flex items-start gap-2">
            <Server size={14} className="text-gray-400 mt-0.5 shrink-0" />
            <div>
              <p className="text-xs text-gray-500">Listen</p>
              <p className="text-sm font-mono">{status?.address || '-'}:{status?.port || '-'}</p>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <Clock size={14} className="text-gray-400 mt-0.5 shrink-0" />
            <div>
              <p className="text-xs text-gray-500">Uptime</p>
              <p className="text-sm font-mono">{status ? formatUptime(status.uptime_seconds) : '-'}</p>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <Hash size={14} className="text-gray-400 mt-0.5 shrink-0" />
            <div>
              <p className="text-xs text-gray-500">PID</p>
              <p className="text-sm font-mono">{status?.pid || '-'}</p>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <Activity size={14} className="text-gray-400 mt-0.5 shrink-0" />
            <div>
              <p className="text-xs text-gray-500">Version</p>
              <p className="text-sm font-mono">{status?.version || '-'}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Config & Stats Card */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div className="bg-white rounded-lg border border-gray-200 p-5">
          <h2 className="text-base font-semibold text-gray-900 flex items-center gap-2 mb-3">
            <FileText size={18} />
            Configuration
          </h2>
          <dl className="space-y-2 text-sm">
            <div>
              <dt className="text-xs text-gray-500">Config Path</dt>
              <dd className="font-mono text-gray-700 truncate" title={status?.config_path}>{status?.config_path || '-'}</dd>
            </div>
            <div>
              <dt className="text-xs text-gray-500">Started At</dt>
              <dd className="text-gray-700">{status ? formatTime(status.started_at) : '-'}</dd>
            </div>
            <div>
              <dt className="text-xs text-gray-500">Model Routes</dt>
              <dd className="text-gray-700">{totalRoutes} total, {enabledModels.length} enabled</dd>
            </div>
          </dl>
        </div>
        <div className="bg-white rounded-lg border border-gray-200 p-5">
          <h2 className="text-base font-semibold text-gray-900 flex items-center gap-2 mb-3">
            <Activity size={18} />
            Request Stats
          </h2>
          <dl className="space-y-2 text-sm">
            <div>
              <dt className="text-xs text-gray-500">Total Requests</dt>
              <dd className="text-gray-700">{status?.total_requests ?? '-'}</dd>
            </div>
            <div>
              <dt className="text-xs text-gray-500">Success / Failed</dt>
              <dd className="text-gray-700">
                <span className="text-green-600">{status?.success_requests ?? 0}</span>
                {' / '}
                <span className="text-red-600">{status?.failed_requests ?? 0}</span>
              </dd>
            </div>
            <div>
              <dt className="text-xs text-gray-500">Active Connections</dt>
              <dd className="text-gray-700">{status?.active_connections ?? '-'}</dd>
            </div>
          </dl>
        </div>
      </div>

      {/* Actions Card */}
      <div className="bg-white rounded-lg border border-gray-200 p-5">
        <h2 className="text-base font-semibold text-gray-900 mb-3">Actions</h2>
        <div className="flex gap-3">
          <button
            onClick={handleReload}
            disabled={reloading || restarting}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium bg-white border border-gray-300 text-gray-700 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <RefreshCw size={14} className={reloading ? 'animate-spin' : ''} />
            {reloading ? 'Reloading...' : 'Reload Config'}
          </button>
          <button
            onClick={handleRestart}
            disabled={reloading || restarting}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium bg-white border border-gray-300 text-gray-700 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <RotateCw size={14} className={restarting ? 'animate-spin' : ''} />
            {restarting ? 'Restarting...' : 'Restart Server'}
          </button>
        </div>
      </div>
    </div>
  )
}
