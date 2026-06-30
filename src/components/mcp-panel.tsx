import { useEffect, useMemo, useState } from 'react'
import { Plus, Trash2, Save, RefreshCw, Power, Wifi, Check, X, ChevronDown, ChevronRight } from 'lucide-react'
import { loadConfig, saveConfig, testMcpConnection } from '@/lib/ipc'
import type { McpServerConfig } from '@/types/ipc'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'

function toEnvString(env?: Record<string, string>): string {
  if (!env) return ''
  return Object.entries(env)
    .map(([k, v]) => `${k}=${v}`)
    .join('\n')
}

function parseEnvString(text: string): Record<string, string> {
  const env: Record<string, string> = {}
  for (const raw of text.split('\n')) {
    const line = raw.trim()
    if (!line) continue
    const idx = line.indexOf('=')
    if (idx <= 0) continue
    const key = line.slice(0, idx).trim()
    const value = line.slice(idx + 1).trim()
    if (key) env[key] = value
  }
  return env
}

function makeId(): string {
  return `mcp-${Math.random().toString(36).slice(2, 10)}`
}

export function McpPanel() {
  const [servers, setServers] = useState<McpServerConfig[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [envDraft, setEnvDraft] = useState<Record<string, string>>({})
  const [headersDraft, setHeadersDraft] = useState<Record<string, string>>({})
  const [testResults, setTestResults] = useState<Record<string, { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }>>({})
  const [collapsedServers, setCollapsedServers] = useState<Record<string, boolean>>({})

  const hasChanges = useMemo(() => !loading, [loading])

  const reload = async (collapseOnLoad = false) => {
    setLoading(true)
    setError(null)
    try {
      const cfg = await loadConfig()
      const list = cfg.mcpServers ?? []
      setServers(list)
      const envById: Record<string, string> = {}
      const headersById: Record<string, string> = {}
      for (const s of list) {
        envById[s.id] = toEnvString(s.env)
        headersById[s.id] = toEnvString(s.headers)
      }
      setEnvDraft(envById)
      setHeadersDraft(headersById)
      setCollapsedServers((prev) => {
        const next: Record<string, boolean> = {}
        for (const s of list) {
          next[s.id] = collapseOnLoad ? true : (prev[s.id] ?? false)
        }
        return next
      })
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    reload(true)
  }, [])

  const updateServer = (id: string, updates: Partial<McpServerConfig>) => {
    setServers((prev) => prev.map((s) => (s.id === id ? { ...s, ...updates } : s)))
  }

  const addServer = () => {
    const id = makeId()
    setServers((prev) => [
      ...prev,
      {
        id,
        name: 'New MCP Server',
        transport: 'stdio',
        command: '',
        args: [],
        url: '',
        enabled: true,
        env: {},
        headers: {},
      },
    ])
    setEnvDraft((prev) => ({ ...prev, [id]: '' }))
    setHeadersDraft((prev) => ({ ...prev, [id]: '' }))
    setCollapsedServers((prev) => ({ ...prev, [id]: false }))
  }

  const deleteServer = (id: string) => {
    setServers((prev) => prev.filter((s) => s.id !== id))
    setEnvDraft((prev) => {
      const next = { ...prev }
      delete next[id]
      return next
    })
    setHeadersDraft((prev) => {
      const next = { ...prev }
      delete next[id]
      return next
    })
  }

  const saveAll = async () => {
    setSaving(true)
    setError(null)
    try {
      const payload = servers.map((s) => ({
        ...s,
        transport: s.transport ?? 'stdio',
        args: s.args.filter(Boolean),
        url: (s.url ?? '').trim() || undefined,
        env: parseEnvString(envDraft[s.id] ?? ''),
        headers: parseEnvString(headersDraft[s.id] ?? ''),
      }))
      await saveConfig({ mcpServers: payload })
      await reload()
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const testServer = async (server: McpServerConfig) => {
    setTestResults((prev) => ({ ...prev, [server.id]: { status: 'testing' } }))
    try {
      const message = await testMcpConnection({
        ...server,
        transport: server.transport ?? 'stdio',
        args: server.args ?? [],
        url: (server.url ?? '').trim() || undefined,
        env: parseEnvString(envDraft[server.id] ?? ''),
        headers: parseEnvString(headersDraft[server.id] ?? ''),
      })
      setTestResults((prev) => ({ ...prev, [server.id]: { status: 'ok', message } }))
    } catch (e) {
      const raw = String(e)
      const friendly = raw.includes('test_mcp_connection') && raw.toLowerCase().includes('not found')
        ? 'MCP test command is unavailable in the running app backend. Restart the Tauri app to load the latest Rust commands.'
        : raw
      setTestResults((prev) => ({ ...prev, [server.id]: { status: 'error', message: friendly } }))
    }
  }

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[#6B7B8D]">
        Loading MCP settings...
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <div>
          <h3 className="text-sm font-semibold">MCP Servers</h3>
          <p className="text-xs text-[#6B7B8D]">Configure Model Context Protocol server entries</p>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={() => { void reload() }} title="Reload">
            <RefreshCw className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={addServer} title="Add server">
            <Plus className="h-4 w-4" />
          </Button>
          <Button size="sm" onClick={saveAll} disabled={saving || !hasChanges}>
            <Save className="mr-1 h-4 w-4" />
            {saving ? 'Saving...' : 'Save'}
          </Button>
        </div>
      </div>

      {error && (
        <div className="border-b border-border bg-red-50 px-3 py-2 text-xs text-red-600">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-3 space-y-3">
        {servers.length === 0 && (
          <div className="rounded-md border border-dashed border-border p-4 text-center text-sm text-[#6B7B8D]">
            No MCP servers yet. Click + to add one.
          </div>
        )}

        {servers.map((server) => (
          <div key={server.id} className="rounded-lg border border-border p-3 space-y-3">
            {(() => {
              const isLocal = (server.transport ?? 'stdio') === 'stdio'
              const tr = testResults[server.id]
              const isCollapsed = !!collapsedServers[server.id]
              return (
                <>
            <div className="flex items-center justify-between gap-2">
              <button
                onClick={() => setCollapsedServers((prev) => ({ ...prev, [server.id]: !prev[server.id] }))}
                className="flex items-center gap-2 min-w-0 text-left"
              >
                {isCollapsed ? <ChevronRight className="h-4 w-4 text-[#6B7B8D]" /> : <ChevronDown className="h-4 w-4 text-[#6B7B8D]" />}
                <span className="text-sm font-medium truncate">{server.name || 'Unnamed MCP Server'}</span>
              </button>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => testServer(server)}
                  disabled={tr?.status === 'testing'}
                  title="Test MCP server"
                >
                  {tr?.status === 'testing' ? (
                    <span className="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />
                  ) : (
                    <Wifi className="h-3.5 w-3.5" />
                  )}
                  <span className="ml-1">Test</span>
                </Button>
                <div className="flex items-center gap-1 text-xs text-[#6B7B8D]">
                  <Power className="h-3.5 w-3.5" />
                  <span>{server.enabled ? 'On' : 'Off'}</span>
                </div>
                <Switch
                  checked={server.enabled}
                  onCheckedChange={(checked) => updateServer(server.id, { enabled: checked })}
                />
                <Button variant="ghost" size="sm" onClick={() => deleteServer(server.id)} title="Delete server">
                  <Trash2 className="h-4 w-4 text-red-500" />
                </Button>
              </div>
            </div>

            {tr && tr.status !== 'idle' && tr.status !== 'testing' && (
              <div className={`flex items-center gap-1 text-xs ${tr.status === 'ok' ? 'text-green-600' : 'text-red-600'}`}>
                {tr.status === 'ok' ? <Check className="h-3.5 w-3.5" /> : <X className="h-3.5 w-3.5" />}
                <span>{tr.message}</span>
              </div>
            )}

            {!isCollapsed && (
              <>

            <div className="space-y-1">
              <Label>MCP Name</Label>
              <Input
                value={server.name}
                onChange={(e) => updateServer(server.id, { name: e.target.value })}
                placeholder="Server name"
              />
            </div>

            <div className="space-y-1">
              <Label>Mode</Label>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => updateServer(server.id, { transport: 'stdio' })}
                  className={`px-3 py-1.5 text-xs rounded-md border transition-colors ${
                    isLocal
                      ? 'bg-[#4A90D9]/10 border-[#4A90D9] text-[#1A2332]'
                      : 'border-border text-[#6B7B8D] hover:bg-[#E8EDF2]'
                  }`}
                >
                  Local (stdio)
                </button>
                <button
                  type="button"
                  onClick={() => {
                    const next = server.transport === 'sse' ? 'sse' : 'http'
                    updateServer(server.id, { transport: next })
                  }}
                  className={`px-3 py-1.5 text-xs rounded-md border transition-colors ${
                    !isLocal
                      ? 'bg-[#4A90D9]/10 border-[#4A90D9] text-[#1A2332]'
                      : 'border-border text-[#6B7B8D] hover:bg-[#E8EDF2]'
                  }`}
                >
                  Remote (HTTP/SSE)
                </button>
              </div>
            </div>

            {isLocal ? (
              <>
                <div className="space-y-1">
                  <Label>Command</Label>
                  <Input
                    value={server.command}
                    onChange={(e) => updateServer(server.id, { command: e.target.value })}
                    placeholder="npx -y @modelcontextprotocol/server-filesystem"
                  />
                </div>

                <div className="space-y-1">
                  <Label>Args (space-separated)</Label>
                  <Input
                    value={server.args.join(' ')}
                    onChange={(e) => updateServer(server.id, { args: e.target.value.split(' ').map((s) => s.trim()).filter(Boolean) })}
                    placeholder="--path D:/work/MyAgent"
                  />
                </div>

                <div className="space-y-1">
                  <Label>Environment (KEY=VALUE per line)</Label>
                  <textarea
                    className="w-full min-h-20 rounded-md border border-input bg-background px-3 py-2 text-sm resize-y"
                    value={envDraft[server.id] ?? ''}
                    onChange={(e) => setEnvDraft((prev) => ({ ...prev, [server.id]: e.target.value }))}
                    placeholder="NODE_ENV=production"
                  />
                </div>
              </>
            ) : (
              <>
                <div className="space-y-1">
                  <Label>Remote Transport</Label>
                  <select
                    value={server.transport === 'sse' ? 'sse' : 'http'}
                    onChange={(e) => updateServer(server.id, { transport: e.target.value as 'http' | 'sse' })}
                    className="w-full h-9 rounded-md border border-input bg-background px-3 text-sm"
                  >
                    <option value="http">HTTP</option>
                    <option value="sse">SSE</option>
                  </select>
                </div>

                <div className="space-y-1">
                  <Label>Remote URL</Label>
                  <Input
                    value={server.url ?? ''}
                    onChange={(e) => updateServer(server.id, { url: e.target.value })}
                    placeholder="https://your-mcp-host.example.com/mcp"
                  />
                </div>

                <div className="space-y-1">
                  <Label>Headers (KEY=VALUE per line)</Label>
                  <textarea
                    className="w-full min-h-20 rounded-md border border-input bg-background px-3 py-2 text-sm resize-y"
                    value={headersDraft[server.id] ?? ''}
                    onChange={(e) => setHeadersDraft((prev) => ({ ...prev, [server.id]: e.target.value }))}
                    placeholder="Authorization=Bearer YOUR_TOKEN"
                  />
                </div>
              </>
            )}
              </>
            )}
                </>
              )
            })()}
          </div>
        ))}
      </div>
    </div>
  )
}
