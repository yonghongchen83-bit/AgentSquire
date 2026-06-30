import type { ProviderInfo } from '@/types/ipc'

export function resolveModelSelection(
  providers: ProviderInfo[],
  stateProvider: string,
  stateModel: string,
  storedProvider: string,
  storedModel: string,
): { selectedProvider: string; selectedModel: string } {
  const firstProvider = providers[0]
  const firstProviderName = firstProvider?.name || ''
  const firstModel = firstProvider?.default_model || firstProvider?.models[0] || ''

  const hasValidPair = (providerName: string, modelName: string) => {
    if (!providerName || !modelName) return false
    const provider = providers.find((p) => p.name === providerName)
    return !!provider && provider.models.includes(modelName)
  }

  let selectedProvider = firstProviderName
  let selectedModel = firstModel

  if (hasValidPair(stateProvider, stateModel)) {
    selectedProvider = stateProvider
    selectedModel = stateModel
  } else if (hasValidPair(storedProvider, storedModel)) {
    selectedProvider = storedProvider
    selectedModel = storedModel
  }

  return { selectedProvider, selectedModel }
}
