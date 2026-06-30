import { useCallback, useEffect, useRef, useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { testConnection, fetchModels } from '@/lib/ipc'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import {
  Tabs, TabsContent, TabsList, TabsTrigger,
} from '@/components/ui/tabs'
import { useSettingsStore } from '@/stores/settings-store'
import type { AppConfig, LlmProviderConfig } from '@/types/ipc'
import { getProvider } from '@/components/settings/provider-catalog'
import { applyThemeClass } from '@/components/settings/theme-utils'
import { GeneralTab } from '@/components/settings/GeneralTab'
import { SearchTab } from '@/components/settings/SearchTab'
import { TerminalTab } from '@/components/settings/TerminalTab'
import { LlmTab } from '@/components/settings/LlmTab'

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

  const { data: fetchedConfig } = useQuery({
    queryKey: ['config'],
    queryFn: () => invoke<AppConfig>('get_config'),
    enabled: open,
  })

  useEffect(() => {
    if (open && fetchedConfig && !loadedRef.current) {
      if (!Array.isArray(fetchedConfig.llmProviders)) {
        return
      }
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

          <TabsContent value="general">
            <GeneralTab
              config={config}
              handleThemeChange={handleThemeChange}
              updateEditorFontSize={updateEditorFontSize}
              updateEditorWordWrap={updateEditorWordWrap}
              updateEditorTabSize={updateEditorTabSize}
            />
          </TabsContent>

          <TabsContent value="llm">
            <LlmTab
              config={config}
              testResults={testResults}
              modelTestResults={modelTestResults}
              fetchedModels={fetchedModels}
              fetchingModels={fetchingModels}
              showCustomModel={showCustomModel}
              selectedProviderId={selectedProviderId}
              collapsedProviders={collapsedProviders}
              onToggleCollapse={(index) => setCollapsedProviders((prev) => ({ ...prev, [index]: !prev[index] }))}
              onRemoveProvider={removeLlmProvider}
              onProviderSelect={handleProviderSelect}
              onUpdateProvider={updateLlmProvider}
              onAddModel={handleAddModel}
              onRemoveModel={handleRemoveModel}
              onCustomModelAdd={handleCustomModelAdd}
              onCancelCustomModel={(index) => setShowCustomModel((prev) => ({ ...prev, [index]: false }))}
              onRefreshModels={handleRefreshModels}
              onTestConnection={handleTestConnection}
              onTestModel={handleTestModel}
              onAddProvider={handleAddProvider}
              onVerboseLoggingChange={updateVerboseLogging}
            />
          </TabsContent>

          <TabsContent value="search">
            <SearchTab
              config={config}
              updateSearchExclude={updateSearchExclude}
            />
          </TabsContent>

          <TabsContent value="terminal">
            <TerminalTab
              config={config}
              updateTerminalShell={updateTerminalShell}
              updateTerminalFontSize={updateTerminalFontSize}
            />
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
