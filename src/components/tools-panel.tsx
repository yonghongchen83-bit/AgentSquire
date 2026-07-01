import { useEffect, useMemo, useState } from 'react'
import { listAvailableTools, loadConfig, saveConfig } from '@/lib/ipc'
import type { ToolInfo } from '@/types/ipc'
import { Switch } from '@/components/ui/switch'
import { ScrollArea } from '@/components/ui/scroll-area'
import { RefreshCw, Wrench, Puzzle, ChevronDown, ChevronRight, Shield, ShieldAlert } from 'lucide-react'

interface ToolCategory {
  name: string
  icon: typeof Wrench
  tools: ToolInfo[]
}

export function ToolsPanel() {
  const [tools, setTools] = useState<ToolInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})

  const reload = async () => {
    setLoading(true)
    try {
      const result = await listAvailableTools()
      setTools(result)
    } catch {
      setTools([])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    reload()
  }, [])

  // Organize into categories
  const categories: ToolCategory[] = useMemo(() => {
    const systemTools = tools.filter((t) => t.category === 'system')
    const mcpTools = tools.filter((t) => t.category === 'mcp')

    const cats: ToolCategory[] = []
    if (systemTools.length > 0) {
      cats.push({ name: 'System Tools', icon: Wrench, tools: systemTools })
    }

    // Group MCP tools by server
    const mcpByServer: Record<string, ToolInfo[]> = {}
    for (const t of mcpTools) {
      const server = t.serverName || 'Unknown Server'
      if (!mcpByServer[server]) mcpByServer[server] = []
      mcpByServer[server].push(t)
    }

    for (const [server, serverTools] of Object.entries(mcpByServer)) {
      cats.push({ name: `MCP: ${server}`, icon: Puzzle, tools: serverTools })
    }

    return cats
  }, [tools])

  const toggleTool = async (toolName: string, enabled: boolean) => {
    setSaving(true)
    try {
      const config = await loadConfig()
      const disabled = config.disabledTools ?? []
      const newDisabled = enabled
        ? disabled.filter((d) => d !== toolName)
        : [...disabled, toolName]
      await saveConfig({ disabledTools: newDisabled })
      // Optimistically update local state
      setTools((prev) =>
        prev.map((t) => (t.name === toolName ? { ...t, enabled } : t)),
      )
    } catch {
      // Revert on error
      void reload()
    } finally {
      setSaving(false)
    }
  }

  const toggleCategory = async (catTools: ToolInfo[], enabled: boolean) => {
    setSaving(true)
    try {
      const config = await loadConfig()
      const disabled = config.disabledTools ?? []
      const catNames = new Set(catTools.map((t) => t.name))
      let newDisabled: string[]
      if (enabled) {
        // Enable the category — remove all category tool names from disabled list
        newDisabled = disabled.filter((d) => !catNames.has(d))
      } else {
        // Disable the category — add all category tool names that aren't already disabled
        const existing = new Set(disabled)
        newDisabled = [...disabled]
        for (const name of catNames) {
          if (!existing.has(name)) newDisabled.push(name)
        }
      }
      await saveConfig({ disabledTools: newDisabled })
      // Optimistically update local state
      setTools((prev) =>
        prev.map((t) =>
          catNames.has(t.name) ? { ...t, enabled } : t,
        ),
      )
    } catch {
      void reload()
    } finally {
      setSaving(false)
    }
  }

  const toggleCollapse = (key: string) => {
    setCollapsed((prev) => ({ ...prev, [key]: !prev[key] }))
  }

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[#6B7B8D]">
        Loading tools...
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <div>
          <h3 className="text-sm font-semibold">Tools</h3>
          <p className="text-xs text-[#6B7B8D]">Enable/disable tools available to AI</p>
        </div>
        <button
          onClick={() => { void reload() }}
          className="flex items-center justify-center w-7 h-7 rounded hover:bg-[#D0DCE8] text-[#6B7B8D] hover:text-[#1A2332]"
          title="Refresh tool list"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {tools.length === 0 && (
        <div className="flex flex-col items-center justify-center flex-1 gap-2 text-[#6B7B8D] p-4">
          <Wrench className="h-8 w-8" />
          <p className="text-sm">No tools available</p>
          <p className="text-xs text-center">
            System tools and MCP server tools will appear here.
          </p>
        </div>
      )}

      <ScrollArea className="flex-1">
        <div className="p-2 space-y-1">
          {categories.map((cat) => {
            const isCollapsed = collapsed[cat.name] ?? false
            const enabledCount = cat.tools.filter((t) => t.enabled).length
            const allEnabled = enabledCount === cat.tools.length
            return (
              <div key={cat.name}>
                <div className="flex items-center gap-1.5 w-full px-2 py-1 rounded-sm hover:bg-muted transition-colors group">
                  <button
                    onClick={() => toggleCollapse(cat.name)}
                    className="flex items-center gap-1.5 flex-1 min-w-0"
                  >
                    {isCollapsed ? (
                      <ChevronRight className="h-3.5 w-3.5 shrink-0 text-[#6B7B8D]" />
                    ) : (
                      <ChevronDown className="h-3.5 w-3.5 shrink-0 text-[#6B7B8D]" />
                    )}
                    <cat.icon className="h-3.5 w-3.5 shrink-0 text-[#6B7B8D]" />
                    <span className="text-xs font-semibold text-[#6B7B8D] truncate">{cat.name}</span>
                  </button>
                  <span className="text-[10px] text-[#6B7B8D] opacity-60 mr-1 shrink-0">
                    {enabledCount}/{cat.tools.length}
                  </span>
                  <Switch
                    checked={allEnabled}
                    disabled={saving}
                    onCheckedChange={(checked) => toggleCategory(cat.tools, checked)}
                    className="data-[state=checked]:bg-[#4A90D9] h-4 w-7 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                  />
                </div>

                {!isCollapsed && (
                  <div className="ml-2 space-y-0.5">
                    {cat.tools.map((tool) => (
                      <div
                        key={tool.name}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-sm hover:bg-muted/50 group"
                      >
                        <Switch
                          checked={tool.enabled}
                          disabled={saving}
                          onCheckedChange={(checked) => toggleTool(tool.name, checked)}
                          className="data-[state=checked]:bg-[#4A90D9]"
                        />
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-1.5">
                            <span className={`text-xs font-medium truncate ${
                              tool.enabled ? 'text-foreground' : 'text-[#6B7B8D]'
                            }`}>
                              {tool.name}
                            </span>
                            {tool.danger === 'destructive' ? (
                              <ShieldAlert className="h-3 w-3 text-amber-500 shrink-0" title="Destructive tool" />
                            ) : (
                              <Shield className="h-3 w-3 text-green-500 shrink-0" title="Safe tool" />
                            )}
                          </div>
                          <p className={`text-[11px] leading-tight truncate ${
                            tool.enabled ? 'text-[#6B7B8D]' : 'text-[#6B7B8D]/50'
                          }`}>
                            {tool.description}
                          </p>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </ScrollArea>

      {tools.length > 0 && (
        <div className="border-t border-border px-3 py-1.5 text-[11px] text-[#6B7B8D]">
          {tools.filter((t) => t.enabled).length} of {tools.length} tools enabled
        </div>
      )}
    </div>
  )
}
