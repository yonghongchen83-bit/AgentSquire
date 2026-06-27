import { useCallback, useEffect } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { Sun, Moon, Monitor, Plus, Trash2 } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Tabs, TabsContent, TabsList, TabsTrigger,
} from '@/components/ui/tabs'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { useSettingsStore } from '@/stores/settings-store'
import type { AppConfig } from '@/types/ipc'

const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor }

function ThemeCard({ mode, current, onChange }: { mode: 'light' | 'dark' | 'system'; current: string; onChange: (v: string) => void }) {
  const Icon = THEME_ICONS[mode]
  const isActive = current === mode
  return (
    <button
      onClick={() => onChange(mode)}
      className={`flex flex-col items-center gap-2 p-4 rounded-lg border-2 transition-all ${
        isActive
          ? 'border-primary bg-primary/5'
          : 'border-border hover:border-muted-foreground/30'
      }`}
    >
      <Icon className={`h-6 w-6 ${isActive ? 'text-primary' : 'text-muted-foreground'}`} />
      <span className={`text-sm font-medium capitalize ${isActive ? 'text-foreground' : 'text-muted-foreground'}`}>
        {mode}
      </span>
    </button>
  )
}

export function SettingsDialog() {
  const open = useSettingsStore((s) => s.open)
  const setOpen = useSettingsStore((s) => s.setOpen)
  const config = useSettingsStore((s) => s.config)
  const setConfig = useSettingsStore((s) => s.setConfig)
  const updateTheme = useSettingsStore((s) => s.updateTheme)
  const updateEditorFontSize = useSettingsStore((s) => s.updateEditorFontSize)
  const updateEditorTabSize = useSettingsStore((s) => s.updateEditorTabSize)
  const updateEditorWordWrap = useSettingsStore((s) => s.updateEditorWordWrap)
  const updateTerminalFontSize = useSettingsStore((s) => s.updateTerminalFontSize)
  const updateTerminalShell = useSettingsStore((s) => s.updateTerminalShell)
  const updateSearchExclude = useSettingsStore((s) => s.updateSearchExclude)
  const updateLlmProvider = useSettingsStore((s) => s.updateLlmProvider)
  const addLlmProvider = useSettingsStore((s) => s.addLlmProvider)
  const removeLlmProvider = useSettingsStore((s) => s.removeLlmProvider)

  const { data: fetchedConfig } = useQuery({
    queryKey: ['config'],
    queryFn: () => invoke<AppConfig>('get_config'),
    enabled: open,
  })

  useEffect(() => {
    if (fetchedConfig && !config) {
      setConfig(fetchedConfig)
    }
  }, [fetchedConfig, config, setConfig])

  const saveMutation = useMutation({
    mutationFn: (cfg: AppConfig) => invoke('save_config', { newConfig: cfg }),
    onSuccess: () => setOpen(false),
  })

  const handleSave = useCallback(() => {
    if (config) saveMutation.mutate(config)
  }, [config, saveMutation])

  const handleThemeChange = (value: string) => {
    updateTheme(value as 'light' | 'dark' | 'system')
    applyThemeClass(value as 'light' | 'dark' | 'system')
  }

  if (!config) return null

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Configure your editor, LLM providers, search, and terminal
          </DialogDescription>
        </DialogHeader>

        <Tabs defaultValue="general" className="mt-4">
          <TabsList className="grid grid-cols-4 w-full">
            <TabsTrigger value="general">General</TabsTrigger>
            <TabsTrigger value="llm">LLM</TabsTrigger>
            <TabsTrigger value="search">Search</TabsTrigger>
            <TabsTrigger value="terminal">Terminal</TabsTrigger>
          </TabsList>

          <TabsContent value="general" className="space-y-6">
            <div className="space-y-3">
              <Label>Theme</Label>
              <div className="flex gap-3">
                {(['light', 'dark', 'system'] as const).map((mode) => (
                  <ThemeCard
                    key={mode}
                    mode={mode}
                    current={config.theme}
                    onChange={handleThemeChange}
                  />
                ))}
              </div>
            </div>

            <div className="space-y-3">
              <Label>Editor Font Size</Label>
              <div className="flex items-center gap-2">
                <Input
                  type="number"
                  min={10}
                  max={32}
                  value={config.fontSize}
                  onChange={(e) => updateEditorFontSize(Number(e.target.value))}
                  className="w-20"
                />
                <span className="text-sm text-muted-foreground">px</span>
              </div>
            </div>

            <div className="space-y-3">
              <Label>Word Wrap</Label>
              <div className="flex items-center gap-2">
                <Switch
                  checked={config.wordWrap ?? false}
                  onCheckedChange={updateEditorWordWrap}
                />
                <span className="text-sm text-muted-foreground">
                  {config.wordWrap ? 'Enabled' : 'Disabled'}
                </span>
              </div>
            </div>

            <div className="space-y-3">
              <Label>Tab Size</Label>
              <Select
                value={String(config.tabSize ?? 4)}
                onValueChange={(v) => updateEditorTabSize(Number(v))}
              >
                <SelectTrigger className="w-20">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {[2, 4, 6, 8].map((n) => (
                    <SelectItem key={n} value={String(n)}>{n}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </TabsContent>

          <TabsContent value="llm" className="space-y-4">
            {config.llmProviders.length === 0 && (
              <p className="text-sm text-muted-foreground py-4 text-center">
                No LLM providers configured. Add one to get started.
              </p>
            )}
            {config.llmProviders.map((provider, i) => (
              <div key={i} className="space-y-3 p-4 rounded-lg border border-border relative">
                <button
                  onClick={() => removeLlmProvider(i)}
                  className="absolute top-2 right-2 p-1 text-muted-foreground hover:text-destructive transition-colors"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Name</Label>
                    <Input
                      value={provider.name}
                      onChange={(e) => updateLlmProvider(i, { name: e.target.value })}
                      placeholder="My OpenAI"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Model</Label>
                    <Input
                      value={provider.model}
                      onChange={(e) => updateLlmProvider(i, { model: e.target.value })}
                      placeholder="gpt-4"
                    />
                  </div>
                  <div className="space-y-1.5 col-span-2">
                    <Label>API Key</Label>
                    <Input
                      type="password"
                      value={provider.apiKey}
                      onChange={(e) => updateLlmProvider(i, { apiKey: e.target.value })}
                      placeholder="sk-..."
                    />
                  </div>
                  <div className="space-y-1.5 col-span-2">
                    <Label>Endpoint (optional)</Label>
                    <Input
                      value={provider.endpoint ?? ''}
                      onChange={(e) => updateLlmProvider(i, { endpoint: e.target.value })}
                      placeholder="https://api.openai.com/v1"
                    />
                  </div>
                </div>
              </div>
            ))}
            <Button variant="outline" size="sm" onClick={addLlmProvider} className="w-full">
              <Plus className="h-4 w-4 mr-2" />
              Add Provider
            </Button>
          </TabsContent>

          <TabsContent value="search" className="space-y-4">
            <div className="space-y-3">
              <Label>Exclude Patterns</Label>
              <p className="text-xs text-muted-foreground">
                One pattern per line. These directories are skipped during search.
              </p>
              <textarea
                className="w-full h-24 rounded-md border border-input bg-background px-3 py-2 text-sm resize-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                value={config.searchExclude?.join('\n') ?? ''}
                onChange={(e) => updateSearchExclude(e.target.value.split('\n').filter(Boolean))}
                placeholder="node_modules&#10;.git&#10;target&#10;dist"
              />
            </div>
          </TabsContent>

          <TabsContent value="terminal" className="space-y-4">
            <div className="space-y-3">
              <Label>Shell Path</Label>
              <Input
                value={config.terminalShell ?? ''}
                onChange={(e) => updateTerminalShell(e.target.value)}
                placeholder="e.g. /bin/bash, C:\Windows\System32\cmd.exe"
              />
              <p className="text-xs text-muted-foreground">
                Leave empty to use system default shell.
              </p>
            </div>

            <div className="space-y-3">
              <Label>Terminal Font Size</Label>
              <div className="flex items-center gap-2">
                <Input
                  type="number"
                  min={10}
                  max={32}
                  value={config.terminalFontSize ?? 13}
                  onChange={(e) => updateTerminalFontSize(Number(e.target.value))}
                  className="w-20"
                />
                <span className="text-sm text-muted-foreground">px</span>
              </div>
            </div>
          </TabsContent>
        </Tabs>

        <div className="flex justify-end gap-2 mt-4 pt-4 border-t border-border">
          <Button variant="outline" onClick={() => setOpen(false)}>Cancel</Button>
          <Button onClick={handleSave} disabled={saveMutation.isPending}>
            {saveMutation.isPending ? 'Saving...' : 'Save'}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function applyThemeClass(theme: string) {
  const root = document.documentElement
  if (theme === 'dark') {
    root.classList.add('dark')
  } else if (theme === 'light') {
    root.classList.remove('dark')
  } else {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    root.classList.toggle('dark', prefersDark)
  }
}

export function initTheme() {
  invoke<AppConfig>('get_config').then((config) => {
    applyThemeClass(config.theme)
  }).catch(() => {})
}
