import { useCallback, useEffect, useRef, useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Sun, Moon, Monitor, Plus, Trash2, Wifi, Check, X, RefreshCw, ChevronDown, ChevronRight } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { testConnection, fetchModels } from '@/lib/ipc'
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
import type { AppConfig, LlmProviderConfig } from '@/types/ipc'

interface ModelInfo {
  id: string
  label?: string
  metadata?: Record<string, string>
}

interface ProviderCategory {
  id: string
  label: string
  defaultProviderType: string
  defaultEndpoint: string
  knownModels: ModelInfo[]
  canFetchModels: boolean
}

const PROVIDERS: ProviderCategory[] = [
  {
    id: 'openai',
    label: 'ChatGPT',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://api.openai.com/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'gpt-4o', metadata: { max_context: '128000', vision: 'true' } },
      { id: 'gpt-4o-mini', metadata: { max_context: '128000', vision: 'true' } },
      { id: 'gpt-4-turbo', metadata: { max_context: '128000' } },
      { id: 'gpt-3.5-turbo', metadata: { max_context: '16385' } },
      { id: 'o1', metadata: { max_context: '200000' } },
      { id: 'o3-mini', metadata: { max_context: '200000', thinking_format: 'reasoning_content' } },
    ],
  },
  {
    id: 'anthropic',
    label: 'Anthropic',
    defaultProviderType: 'anthropic',
    defaultEndpoint: 'https://api.anthropic.com/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'claude-sonnet-4-20250514', label: 'Claude Sonnet 4', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-opus-4-6', label: 'Claude Opus 4.6', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-opus-4-5', label: 'Claude Opus 4.5', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-haiku-4-5', label: 'Claude Haiku 4.5', metadata: { max_context: '200000' } },
    ],
  },
  {
    id: 'deepseek',
    label: 'DeepSeek',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://api.deepseek.com',
    canFetchModels: false,
    knownModels: [
      { id: 'deepseek-v4-flash', label: 'DeepSeek V4 Flash', metadata: { max_context: '128000', thinking_format: 'reasoning_content' } },
      { id: 'deepseek-v4-pro', label: 'DeepSeek V4 Pro', metadata: { max_context: '128000', thinking_format: 'reasoning_content' } },
      { id: 'deepseek-chat', label: 'DeepSeek Chat (legacy)', metadata: { max_context: '128000' } },
      { id: 'deepseek-reasoner', label: 'DeepSeek Reasoner (legacy)', metadata: { max_context: '128000', thinking_format: 'reasoning_content' } },
    ],
  },
  {
    id: 'ollama',
    label: 'Ollama',
    defaultProviderType: 'openai',
    defaultEndpoint: 'http://localhost:11434/v1',
    canFetchModels: true,
    knownModels: [],
  },
  {
    id: 'opencode-zen',
    label: 'OpenCode Zen',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://opencode.ai/zen/v1/chat/completions',
    canFetchModels: true,
    knownModels: [
      { id: 'gpt-5.5', metadata: {} },
      { id: 'gpt-5.5-pro', metadata: {} },
      { id: 'gpt-5.4', metadata: {} },
      { id: 'gpt-5.4-pro', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'gpt-5.4-mini', metadata: {} },
      { id: 'gpt-5.4-nano', metadata: {} },
      { id: 'gpt-5.3-codex', metadata: {} },
      { id: 'gpt-5.3-codex-spark', metadata: {} },
      { id: 'gpt-5.2', metadata: {} },
      { id: 'gpt-5.2-codex', metadata: {} },
      { id: 'gpt-5.1', metadata: {} },
      { id: 'gpt-5.1-codex', metadata: {} },
      { id: 'gpt-5.1-codex-max', metadata: {} },
      { id: 'gpt-5.1-codex-mini', metadata: {} },
      { id: 'gpt-5', metadata: {} },
      { id: 'gpt-5-codex', metadata: {} },
      { id: 'gpt-5-nano', metadata: {} },
      { id: 'deepseek-v4-flash', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'deepseek-v4-pro', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'qwen3.6-plus', metadata: {} },
      { id: 'qwen3.5-plus', metadata: {} },
      { id: 'minimax-m2.7', metadata: {} },
      { id: 'minimax-m2.5', metadata: {} },
      { id: 'glm-5.2', metadata: {} },
      { id: 'glm-5.1', metadata: {} },
      { id: 'glm-5', metadata: {} },
      { id: 'kimi-k2.6', metadata: {} },
      { id: 'kimi-k2.5', metadata: {} },
      { id: 'grok-build-0.1', metadata: {} },
    ],
  },
  {
    id: 'opencode-zen-free',
    label: 'OpenCode Zen Free',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://opencode.ai/zen/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'gpt-5-nano', metadata: {} },
      { id: 'deepseek-v4-flash-free', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'big-pickle', metadata: {} },
      { id: 'mimo-v2.5-free', metadata: {} },
      { id: 'north-mini-code-free', metadata: {} },
      { id: 'nemotron-3-ultra-free', metadata: {} },
      { id: 'qwen3.6-plus-free', metadata: {} },
      { id: 'minimax-m3-free', metadata: {} },
    ],
  },
  {
    id: 'nvidia-nim',
    label: 'NVIDIA NIM',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://integrate.api.nvidia.com/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'meta/llama-3.1-8b-instruct', metadata: {} },
      { id: 'meta/llama-3.1-70b-instruct', metadata: {} },
      { id: 'nvidia/llama-3.1-nemotron-70b-instruct', metadata: {} },
      { id: 'mistralai/mixtral-8x7b-instruct-v0.1', metadata: {} },
    ],
  },
  {
    id: 'custom',
    label: 'Custom',
    defaultProviderType: 'openai',
    defaultEndpoint: '',
    canFetchModels: false,
    knownModels: [],
  },
]

const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor }

function getProvider(id: string): ProviderCategory | undefined {
  return PROVIDERS.find((p) => p.id === id)
}

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
  const initialTab = useSettingsStore((s) => s.initialTab)
  const updateTheme = useSettingsStore((s) => s.updateTheme)
  const updateEditorFontSize = useSettingsStore((s) => s.updateEditorFontSize)
  const updateEditorTabSize = useSettingsStore((s) => s.updateEditorTabSize)
  const updateEditorWordWrap = useSettingsStore((s) => s.updateEditorWordWrap)
  const updateTerminalFontSize = useSettingsStore((s) => s.updateTerminalFontSize)
  const updateTerminalShell = useSettingsStore((s) => s.updateTerminalShell)
  const updateSearchExclude = useSettingsStore((s) => s.updateSearchExclude)
  const updateVerboseLogging = useSettingsStore((s) => s.updateVerboseLogging)
  const updateLlmProvider = useSettingsStore((s) => s.updateLlmProvider)
  const addLlmProvider = useSettingsStore((s) => s.addLlmProvider)
  const removeLlmProvider = useSettingsStore((s) => s.removeLlmProvider)
  const [testResults, setTestResults] = useState<Record<number, { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }>>({})
  const [modelTestResults, setModelTestResults] = useState<Record<string, { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }>>({})
  const [fetchedModels, setFetchedModels] = useState<Record<number, string[]>>({})
  const [fetchingModels, setFetchingModels] = useState<Record<number, boolean>>({})
  const [showCustomModel, setShowCustomModel] = useState<Record<number, boolean>>({})
  const [selectedProviderId, setSelectedProviderId] = useState<Record<number, string>>({})
  const [collapsedProviders, setCollapsedProviders] = useState<Record<number, boolean>>({})
  const queryClient = useQueryClient()
  const loadedRef = useRef(false)

  const [activeTab, setActiveTab] = useState(initialTab)

  useEffect(() => {
    if (open) {
      setActiveTab(initialTab)
      // Reset loaded state so we reload from backend on each dialog open
      loadedRef.current = false
    }
  }, [open, initialTab])

  const { data: fetchedConfig, refetch: refetchConfig } = useQuery({
    queryKey: ['config'],
    queryFn: () => invoke<AppConfig>('get_config'),
    enabled: open,
  })

  useEffect(() => {
    if (open && fetchedConfig && !loadedRef.current) {
      loadedRef.current = true
      const providers = fetchedConfig.llmProviders.map((p) => ({
        ...p,
        models: p.models?.length ? p.models : (p.model ? [p.model] : []),
      }))
      setConfig({ ...fetchedConfig, llmProviders: providers })
      const initial: Record<number, string> = {}
      providers.forEach((p, i) => {
        if (p.category) initial[i] = p.category
      })
      setSelectedProviderId(initial)
      const initiallyCollapsed: Record<number, boolean> = {}
      providers.forEach((_, i) => {
        initiallyCollapsed[i] = true
      })
      setCollapsedProviders(initiallyCollapsed)
    }
  }, [open, fetchedConfig, setConfig])

  const saveMutation = useMutation({
    mutationFn: (cfg: AppConfig) => invoke('save_config', { newConfig: cfg }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
      setOpen(false)
      emit('providers-changed').catch(() => {})
    },
  })

  const handleSave = useCallback(() => {
    if (config) saveMutation.mutate(config)
  }, [config, saveMutation])

  const handleAddProvider = useCallback(() => {
    const nextIndex = useSettingsStore.getState().config?.llmProviders.length ?? 0
    addLlmProvider()
    setCollapsedProviders((prev) => ({ ...prev, [nextIndex]: false }))
  }, [addLlmProvider])

  const handleThemeChange = (value: string) => {
    updateTheme(value as 'light' | 'dark' | 'system')
    applyThemeClass(value as 'light' | 'dark' | 'system')
  }

  const findApiKeyForProvider = (providerId: string): string => {
    if (!config) return ''
    const existing = config.llmProviders.find((p) => selectedProviderId[config.llmProviders.indexOf(p)] === providerId && p.apiKey)
    return existing?.apiKey ?? ''
  }

  const handleProviderSelect = (index: number, providerId: string) => {
    setSelectedProviderId((prev) => ({ ...prev, [index]: providerId }))
    const prov = getProvider(providerId)
    if (!prov) return
    const existingKey = findApiKeyForProvider(providerId)

    // Preserve existing configured models; only set defaults for a fresh provider
    const currentConfig = useSettingsStore.getState().config
    const existingProvider = currentConfig?.llmProviders[index]
    const hasExistingModels = existingProvider?.models?.length && existingProvider.models.length > 0
    // If switching to a different category, keep the models that are still valid
    const isSameCategory = existingProvider?.category === providerId

    let models: string[]
    let model: string
    let metadata: Record<string, string> | undefined

    if (hasExistingModels && isSameCategory) {
      // Switching back to same category — preserve existing models
      models = existingProvider!.models
      model = existingProvider!.model || existingProvider!.models[0]
      metadata = existingProvider!.metadata
    } else if (hasExistingModels) {
      // Switching to a different category — keep user-added models that intersect with new known models
      const newKnownIds = new Set(prov.knownModels.map((m) => m.id))
      const intersection = existingProvider!.models.filter((m) => newKnownIds.has(m))
      const merged = intersection.length > 0
        ? [...new Set([...prov.knownModels.map((m) => m.id), ...intersection])]
        : prov.knownModels.map((m) => m.id)
      models = merged.filter(Boolean)
      model = models[0] ?? ''
      metadata = models.length > 0 ? prov.knownModels.find((m) => m.id === model)?.metadata : undefined
    } else {
      // Fresh provider — set default first model
      const firstModel = prov.knownModels[0]?.id ?? ''
      models = firstModel ? [firstModel] : []
      model = firstModel
      metadata = prov.knownModels[0]?.metadata
    }

    updateLlmProvider(index, {
      providerType: prov.defaultProviderType,
      model,
      models,
      name: prov.label,
      endpoint: prov.defaultEndpoint,
      apiKey: existingKey,
      metadata,
      category: providerId,
    })
  }

  const handleAddModel = (index: number, modelId: string) => {
    if (modelId === '__custom__') {
      setShowCustomModel((prev) => ({ ...prev, [index]: true }))
      return
    }
    const config = useSettingsStore.getState().config
    if (!config) return
    const provider = config.llmProviders[index]
    if (provider.models.includes(modelId)) return
    const newModels = [...provider.models, modelId]
    updateLlmProvider(index, {
      models: newModels,
      metadata: undefined,
    })
  }

  const handleRemoveModel = (index: number, modelId: string) => {
    const config = useSettingsStore.getState().config
    if (!config) return
    const provider = config.llmProviders[index]
    const newModels = provider.models.filter((m) => m !== modelId)
    updateLlmProvider(index, {
      models: newModels,
      model: provider.model === modelId ? (newModels[0] ?? '') : provider.model,
    })
  }

  const handleCustomModelAdd = (index: number, inputEl: HTMLInputElement | null) => {
    if (!inputEl || !inputEl.value.trim()) return
    handleAddModel(index, inputEl.value.trim())
    inputEl.value = ''
    setShowCustomModel((prev) => ({ ...prev, [index]: false }))
  }

  const handleRefreshModels = async (index: number, provider: LlmProviderConfig) => {
    if (!provider.endpoint) return
    setFetchingModels((prev) => ({ ...prev, [index]: true }))
    try {
      const models = await fetchModels(provider.providerType, provider.endpoint, provider.apiKey)
      setFetchedModels((prev) => ({ ...prev, [index]: models }))
    } catch (e) {
      setTestResults((prev) => ({ ...prev, [index]: { status: 'error', message: String(e) } }))
    } finally {
      setFetchingModels((prev) => ({ ...prev, [index]: false }))
    }
  }

  const handleTestConnection = async (index: number, provider: LlmProviderConfig) => {
    setTestResults((prev) => ({ ...prev, [index]: { status: 'testing' } }))
    const provId = selectedProviderId[index] ?? provider.category ?? ''
    const prov = getProvider(provId)
    const modelToTest = provider.model || prov?.knownModels[0]?.id || provider.providerType
    try {
      const result = await testConnection(provider.providerType, provider.apiKey || '', modelToTest, provider.endpoint)
      setTestResults((prev) => ({ ...prev, [index]: { status: 'ok', message: result } }))
    } catch (e) {
      setTestResults((prev) => ({ ...prev, [index]: { status: 'error', message: String(e) } }))
    }
  }

  const handleTestModel = async (index: number, provider: LlmProviderConfig, modelToTest: string) => {
    const key = `${index}:${modelToTest}`
    setModelTestResults((prev) => ({ ...prev, [key]: { status: 'testing' } }))
    try {
      const result = await testConnection(provider.providerType, provider.apiKey || '', modelToTest, provider.endpoint)
      setModelTestResults((prev) => ({ ...prev, [key]: { status: 'ok', message: result } }))
    } catch (e) {
      setModelTestResults((prev) => ({ ...prev, [key]: { status: 'error', message: String(e) } }))
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => {
      if (!o) {
        // Don't close the dialog if a Select popup (listbox) is open —
        // the Escape key or click should be consumed by the dropdown first
        const openPopup = document.querySelector('[role="listbox"][data-state="open"], [role="dialog"][data-state="open"] ~ [role="listbox"]')
        if (!openPopup) setOpen(false)
      }
    }}>
      {!config ? (
        <DialogContent className="max-w-2xl max-h-[80vh]">
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>Loading configuration...</DialogDescription>
          </DialogHeader>
          <div className="flex items-center justify-center py-12">
            <div className="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin" />
          </div>
        </DialogContent>
      ) : (
      <DialogContent
        className="max-w-2xl max-h-[80vh] overflow-y-auto"
        onPointerDownOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => {
          const openSelect = document.querySelector('[role="listbox"]')
          if (openSelect) e.preventDefault()
        }}
      >
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Configure your editor, LLM providers, search, and terminal
          </DialogDescription>
        </DialogHeader>

        <Tabs value={activeTab} onValueChange={setActiveTab} className="mt-4">
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
            <div className="flex items-center justify-between p-3 rounded-lg border border-border">
              <div className="space-y-0.5">
                <Label>Verbose Logging</Label>
                <p className="text-xs text-muted-foreground">
                  Log all chat request/response payloads to the Output panel (source: chat)
                </p>
              </div>
              <Switch
                checked={config.verboseLogging ?? false}
                onCheckedChange={updateVerboseLogging}
              />
            </div>

            {config.llmProviders.length === 0 && (
              <p className="text-sm text-muted-foreground py-4 text-center">
                No LLM providers configured. Add one to get started.
              </p>
            )}
            {config.llmProviders.map((provider, i) => {
              const tr = testResults[i]
              const provId = selectedProviderId[i] ?? provider.category ?? ''
              const prov = getProvider(provId)
              const knownModels = prov?.knownModels ?? []
              const fetched = fetchedModels[i] ?? []
              const showCustom = showCustomModel[i] ?? false
              const isCollapsed = !!collapsedProviders[i]
              const providerHeading = provider.name || prov?.label || `Provider ${i + 1}`

              return (
              <div key={i} className="space-y-3 p-4 rounded-lg border border-border">
                <div className="flex items-center justify-between gap-2">
                  <button
                    onClick={() => setCollapsedProviders((prev) => ({ ...prev, [i]: !prev[i] }))}
                    className="flex items-center gap-2 text-left min-w-0"
                  >
                    {isCollapsed ? <ChevronRight className="h-4 w-4 text-muted-foreground" /> : <ChevronDown className="h-4 w-4 text-muted-foreground" />}
                    <span className="text-sm font-medium truncate">{providerHeading}</span>
                  </button>
                  <button
                    onClick={() => removeLlmProvider(i)}
                    className="p-1 text-muted-foreground hover:text-destructive transition-colors"
                    title="Delete provider"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>

                {!isCollapsed && (
                  <>

                <div className="space-y-1.5">
                  <Label>Provider</Label>
                  <Select value={provId} onValueChange={(v) => handleProviderSelect(i, v)}>
                    <SelectTrigger>
                      <SelectValue placeholder="Select provider..." />
                    </SelectTrigger>
                    <SelectContent>
                      {PROVIDERS.map((p) => (
                        <SelectItem key={p.id} value={p.id}>{p.label}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                <div className="space-y-1.5">
                  <Label>Display Name</Label>
                  <Input
                    value={provider.name}
                    onChange={(e) => updateLlmProvider(i, { name: e.target.value })}
                    placeholder={prov?.label || 'My Provider'}
                  />
                  <p className="text-xs text-muted-foreground">
                    Appears in chat as "{provider.name || '(name)'} · model"
                  </p>
                </div>

                {provider.model && (
                  <div className="space-y-1.5">
                    <Label>Models</Label>
                    <div className="flex flex-wrap gap-1.5 mb-2">
                      {(provider.models?.length ? provider.models : [provider.model].filter(Boolean)).map((m) => {
                        const mtr = modelTestResults[`${i}:${m}`]
                        return (
                        <span key={m} className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs ${m === provider.model ? 'bg-primary/20 text-primary' : 'bg-muted'}`}>
                          {m}
                          <button
                            onClick={() => handleTestModel(i, provider, m)}
                            className="hover:text-primary leading-none"
                            title={`Test ${m}`}
                          >
                            {mtr?.status === 'testing' ? (
                              <span className="w-2.5 h-2.5 border border-current border-t-transparent rounded-full animate-spin inline-block" />
                            ) : mtr?.status === 'ok' ? (
                              <span className="text-green-600">✓</span>
                            ) : mtr?.status === 'error' ? (
                              <span className="text-red-500" title={mtr.message}>✗</span>
                            ) : (
                              <span className="opacity-50">▶</span>
                            )}
                          </button>
                          <button
                            onClick={() => handleRemoveModel(i, m)}
                            className="hover:text-destructive leading-none"
                          >
                            ×
                          </button>
                        </span>
                      )})}
                    </div>
                    <div className="flex gap-2">
                      <div className="flex-1">
                        {!showCustom ? (
                          <Select value="" onValueChange={(v) => handleAddModel(i, v)}>
                            <SelectTrigger>
                              <SelectValue placeholder="Add model..." />
                            </SelectTrigger>
                            <SelectContent>
                              {knownModels.map((m) => (
                                <SelectItem key={m.id} value={m.id}>{m.label ?? m.id}</SelectItem>
                              ))}
                              {fetched.length > 0 && knownModels.length > 0 && (
                                <SelectItem value="__sep__" disabled>─── fetched ───</SelectItem>
                              )}
                              {fetched.map((m) => (
                                <SelectItem key={m} value={m}>{m}</SelectItem>
                              ))}
                              <SelectItem value="__custom__">Custom model...</SelectItem>
                            </SelectContent>
                          </Select>
                        ) : (
                          <div className="flex gap-2">
                            <Input
                              ref={(el) => {
                                if (el) el.focus()
                              }}
                              placeholder="Enter model name and press Enter..."
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') handleCustomModelAdd(i, e.currentTarget)
                              }}
                            />
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => setShowCustomModel((prev) => ({ ...prev, [i]: false }))}
                            >
                              Cancel
                            </Button>
                          </div>
                        )}
                      </div>
                      {prov?.canFetchModels && provider.endpoint && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleRefreshModels(i, provider)}
                          disabled={fetchingModels[i]}
                          className="shrink-0"
                          title="Refresh models from server"
                        >
                          <RefreshCw className={`h-4 w-4 ${fetchingModels[i] ? 'animate-spin' : ''}`} />
                        </Button>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Primary model highlighted. Add multiple models from the same provider to group them in the chat selector
                    </p>
                  </div>
                )}

                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>API Format</Label>
                    <Select
                      value={provider.providerType}
                      onValueChange={(v) => updateLlmProvider(i, { providerType: v })}
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="openai">OpenAI-compatible</SelectItem>
                        <SelectItem value="anthropic">Anthropic</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-1.5">
                    <Label>Endpoint</Label>
                    <Input
                      value={provider.endpoint ?? ''}
                      onChange={(e) => updateLlmProvider(i, { endpoint: e.target.value })}
                      placeholder={prov?.defaultEndpoint || 'https://...'}
                    />
                  </div>
                </div>

                <div className="space-y-1.5">
                  <Label>API Key</Label>
                  <div className="flex gap-2">
                    <Input
                      type="password"
                      value={provider.apiKey}
                      onChange={(e) => updateLlmProvider(i, { apiKey: e.target.value })}
                      placeholder="sk-..."
                      className="flex-1"
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleTestConnection(i, provider)}
                      disabled={tr?.status === 'testing'}
                      className="shrink-0"
                    >
                      {tr?.status === 'testing' ? (
                        <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                      ) : (
                        <Wifi className="h-4 w-4" />
                      )}
                      <span className="ml-1.5">Test</span>
                    </Button>
                  </div>
                  {tr && tr.status !== 'idle' && tr.status !== 'testing' && (
                    <div className={`flex items-center gap-1 text-xs mt-1 ${
                      tr.status === 'ok' ? 'text-green-600' : 'text-red-500'
                    }`}>
                      {tr.status === 'ok' ? <Check className="h-3 w-3" /> : <X className="h-3 w-3" />}
                      {tr.message}
                    </div>
                  )}
                </div>
                  </>
                )}
              </div>
            )})}
            <Button variant="outline" size="sm" onClick={handleAddProvider} className="w-full">
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
      )}
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
