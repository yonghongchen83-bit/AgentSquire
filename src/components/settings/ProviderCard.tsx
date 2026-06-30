import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import type { LlmProviderConfig } from '@/types/ipc'
import { ChevronDown, ChevronRight, Check, RefreshCw, Trash2, Wifi, X } from 'lucide-react'
import { PROVIDERS, type ProviderCategory } from '@/components/settings/provider-catalog'

export function ProviderCard({
  index,
  provider,
  testResult,
  modelTestResults,
  providerId,
  providerCategory,
  fetched,
  fetching,
  showCustom,
  isCollapsed,
  providerHeading,
  onToggleCollapse,
  onDelete,
  onProviderSelect,
  onUpdateProvider,
  onAddModel,
  onRemoveModel,
  onCustomModelAdd,
  onCancelCustomModel,
  onRefreshModels,
  onTestConnection,
  onTestModel,
}: {
  index: number
  provider: LlmProviderConfig
  testResult?: { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }
  modelTestResults: Record<string, { status: 'idle' | 'testing' | 'ok' | 'error'; message?: string }>
  providerId: string
  providerCategory?: ProviderCategory
  fetched: string[]
  fetching: boolean
  showCustom: boolean
  isCollapsed: boolean
  providerHeading: string
  onToggleCollapse: (index: number) => void
  onDelete: (index: number) => void
  onProviderSelect: (index: number, providerId: string) => void
  onUpdateProvider: (index: number, patch: Partial<LlmProviderConfig>) => void
  onAddModel: (index: number, modelId: string) => void
  onRemoveModel: (index: number, modelId: string) => void
  onCustomModelAdd: (index: number, inputEl: HTMLInputElement | null) => void
  onCancelCustomModel: (index: number) => void
  onRefreshModels: (index: number, provider: LlmProviderConfig) => void
  onTestConnection: (index: number, provider: LlmProviderConfig) => void
  onTestModel: (index: number, provider: LlmProviderConfig, modelToTest: string) => void
}) {
  const knownModels = providerCategory?.knownModels ?? []

  return (
    <div className="space-y-3 p-4 rounded-lg border border-border">
      <div className="flex items-center justify-between gap-2">
        <button
          onClick={() => onToggleCollapse(index)}
          className="flex items-center gap-2 text-left min-w-0"
        >
          {isCollapsed ? <ChevronRight className="h-4 w-4 text-muted-foreground" /> : <ChevronDown className="h-4 w-4 text-muted-foreground" />}
          <span className="text-sm font-medium truncate">{providerHeading}</span>
        </button>
        <button
          onClick={() => onDelete(index)}
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
            <Select value={providerId} onValueChange={(v) => onProviderSelect(index, v)}>
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
              onChange={(e) => onUpdateProvider(index, { name: e.target.value })}
              placeholder={providerCategory?.label || 'My Provider'}
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
                  const mtr = modelTestResults[`${index}:${m}`]
                  return (
                    <span key={m} className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs ${m === provider.model ? 'bg-primary/20 text-primary' : 'bg-muted'}`}>
                      {m}
                      <button
                        onClick={() => onTestModel(index, provider, m)}
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
                        onClick={() => onRemoveModel(index, m)}
                        className="hover:text-destructive leading-none"
                      >
                        ×
                      </button>
                    </span>
                  )
                })}
              </div>
              <div className="flex gap-2">
                <div className="flex-1">
                  {!showCustom ? (
                    <Select value="" onValueChange={(v) => onAddModel(index, v)}>
                      <SelectTrigger>
                        <SelectValue placeholder="Add model..." />
                      </SelectTrigger>
                      <SelectContent>
                        {knownModels.map((m) => (
                          <SelectItem key={m.id} value={m.id}>{m.label ?? m.id}</SelectItem>
                        ))}
                        {fetched.length > 0 && knownModels.length > 0 && (
                          <SelectItem value="__sep__" disabled>--- fetched ---</SelectItem>
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
                          if (e.key === 'Enter') onCustomModelAdd(index, e.currentTarget)
                        }}
                      />
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onCancelCustomModel(index)}
                      >
                        Cancel
                      </Button>
                    </div>
                  )}
                </div>
                {providerCategory?.canFetchModels && provider.endpoint && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => onRefreshModels(index, provider)}
                    disabled={fetching}
                    className="shrink-0"
                    title="Refresh models from server"
                  >
                    <RefreshCw className={`h-4 w-4 ${fetching ? 'animate-spin' : ''}`} />
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
                onValueChange={(v) => onUpdateProvider(index, { providerType: v })}
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
                onChange={(e) => onUpdateProvider(index, { endpoint: e.target.value })}
                placeholder={providerCategory?.defaultEndpoint || 'https://...'}
              />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label>API Key</Label>
            <div className="flex gap-2">
              <Input
                type="password"
                value={provider.apiKey}
                onChange={(e) => onUpdateProvider(index, { apiKey: e.target.value })}
                placeholder="sk-..."
                className="flex-1"
              />
              <Button
                variant="outline"
                size="sm"
                onClick={() => onTestConnection(index, provider)}
                disabled={testResult?.status === 'testing'}
                className="shrink-0"
              >
                {testResult?.status === 'testing' ? (
                  <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                ) : (
                  <Wifi className="h-4 w-4" />
                )}
                <span className="ml-1.5">Test</span>
              </Button>
            </div>
            {testResult && testResult.status !== 'idle' && testResult.status !== 'testing' && (
              <div className={`flex items-center gap-1 text-xs mt-1 ${
                testResult.status === 'ok' ? 'text-green-600' : 'text-red-500'
              }`}>
                {testResult.status === 'ok' ? <Check className="h-3 w-3" /> : <X className="h-3 w-3" />}
                {testResult.message}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  )
}
