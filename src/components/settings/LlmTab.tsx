import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import type { AppConfig, LlmProviderConfig } from '@/types/ipc'
import { getProvider } from '@/components/settings/provider-catalog'
import { ProviderCard } from '@/components/settings/ProviderCard'

export function LlmTab({
  config,
  testResults,
  modelTestResults,
  fetchedModels,
  fetchingModels,
  selectedProviderId,
  collapsedProviders,
  onToggleCollapse,
  onRemoveProvider,
  onProviderSelect,
  onUpdateProvider,
  onAddModel,
  onRemoveModel,
  onRefreshModels,
  onTestConnection,
  onTestModel,
  onAddProvider,
  onVerboseLoggingChange,
}: {
  config: AppConfig
  testResults: Record<number, { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }>
  modelTestResults: Record<string, { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }>
  fetchedModels: Record<number, string[]>
  fetchingModels: Record<number, boolean>
  selectedProviderId: Record<number, string>
  collapsedProviders: Record<number, boolean>
  onToggleCollapse: (index: number) => void
  onRemoveProvider: (index: number) => void
  onProviderSelect: (index: number, providerId: string) => void
  onUpdateProvider: (index: number, patch: Partial<LlmProviderConfig>) => void
  onAddModel: (index: number, modelId: string) => void
  onRemoveModel: (index: number, modelId: string) => void
  onRefreshModels: (index: number, provider: LlmProviderConfig) => void
  onTestConnection: (index: number, provider: LlmProviderConfig) => void
  onTestModel: (index: number, provider: LlmProviderConfig, modelToTest: string) => void
  onAddProvider: () => void
  onVerboseLoggingChange: (value: boolean) => void
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between p-3 rounded-lg border border-border">
        <div className="space-y-0.5">
          <Label>Verbose Logging</Label>
          <p className="text-xs text-muted-foreground">
            Log all chat request/response payloads to the Output panel (source: chat)
          </p>
        </div>
        <Switch
          checked={config.verboseLogging ?? false}
          onCheckedChange={onVerboseLoggingChange}
        />
      </div>

      {config.llmProviders.length === 0 && (
        <p className="text-sm text-muted-foreground py-4 text-center">
          No LLM providers configured. Add one to get started.
        </p>
      )}

      {config.llmProviders.map((provider, i) => {
        const providerId = selectedProviderId[i] ?? provider.category ?? ''
        const providerCategory = getProvider(providerId)
        const providerHeading = provider.name || providerCategory?.label || `Provider ${i + 1}`

        return (
          <ProviderCard
            key={i}
            index={i}
            provider={provider}
            testResult={testResults[i]}
            modelTestResults={modelTestResults}
            providerId={providerId}
            providerCategory={providerCategory}
            fetched={fetchedModels[i] ?? []}
            fetching={!!fetchingModels[i]}
            isCollapsed={!!collapsedProviders[i]}
            providerHeading={providerHeading}
            onToggleCollapse={onToggleCollapse}
            onDelete={onRemoveProvider}
            onProviderSelect={onProviderSelect}
            onUpdateProvider={onUpdateProvider}
            onAddModel={onAddModel}
            onRemoveModel={onRemoveModel}
            onRefreshModels={onRefreshModels}
            onTestConnection={onTestConnection}
            onTestModel={onTestModel}
          />
        )
      })}

      <Button variant="outline" size="sm" onClick={onAddProvider} className="w-full">
        <Plus className="h-4 w-4 mr-2" />
        Add Provider
      </Button>
    </div>
  )
}
