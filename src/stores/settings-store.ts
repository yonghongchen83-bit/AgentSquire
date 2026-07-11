import { create } from 'zustand'
import type { AppConfig } from '@/types/ipc'

interface SettingsStore {
  open: boolean
  config: AppConfig | null
  showSplash: boolean
  initialTab: string
  setOpen: (open: boolean, tab?: string) => void
  setConfig: (config: AppConfig) => void
  setShowSplash: (show: boolean) => void
  updateTheme: (theme: 'light' | 'dark' | 'system') => void
  updateEditorFontSize: (size: number) => void
  updateEditorTabSize: (size: number) => void
  updateEditorWordWrap: (wrap: boolean) => void
  updateTerminalFontSize: (size: number) => void
  updateTerminalShell: (shell: string) => void
  updateSearchExclude: (patterns: string[]) => void
  updateVerboseLogging: (verbose: boolean) => void
  updateLlmProvider: (index: number, updates: Partial<AppConfig['llmProviders'][0]>) => void
  addLlmProvider: () => void
  removeLlmProvider: (index: number) => void
}

const store = create<SettingsStore>((set) => ({
  open: false,
  config: null,
  showSplash: true,
  initialTab: 'general',
  setOpen: (open, tab) => set(() => {
    if (!open) {
      // Reset config when dialog closes so next open reloads from backend
      return { open, initialTab: tab ?? 'general', config: null }
    }
    return { open, initialTab: tab ?? 'general' }
  }),
  setConfig: (config) => set({ config }),
  setShowSplash: (show) => set({ showSplash: show }),
  updateTheme: (theme) => set((s) => ({
    config: s.config ? { ...s.config, theme } : null,
  })),
  updateEditorFontSize: (fontSize) => set((s) => ({
    config: s.config ? { ...s.config, fontSize } : null,
  })),
  updateEditorTabSize: (tabSize) => set((s) => {
    if (!s.config) return {}
    return { config: { ...s.config, tabSize } }
  }),
  updateEditorWordWrap: (wordWrap) => set((s) => {
    if (!s.config) return {}
    return { config: { ...s.config, wordWrap } }
  }),
  updateTerminalFontSize: (terminalFontSize) => set((s) => ({
    config: s.config ? { ...s.config, terminalFontSize } : null,
  })),
  updateTerminalShell: (terminalShell) => set((s) => ({
    config: s.config ? { ...s.config, terminalShell } : null,
  })),
  updateSearchExclude: (searchExclude) => set((s) => ({
    config: s.config ? { ...s.config, searchExclude } : null,
  })),
  updateVerboseLogging: (verboseLogging) => set((s) => ({
    config: s.config ? { ...s.config, verboseLogging } : null,
  })),
  updateLlmProvider: (index, updates) => set((s) => {
    if (!s.config) return {}
    const providers = [...s.config.llmProviders]
    providers[index] = { ...providers[index], ...updates }
    return { config: { ...s.config, llmProviders: providers } }
  }),
  addLlmProvider: () => set((s) => {
    if (!s.config) return {}
    const providers = [...s.config.llmProviders, { providerType: 'openai', name: '', apiKey: '', model: '', phase2Model: '', models: [], endpoint: 'https://api.openai.com/v1' }]
    return { config: { ...s.config, llmProviders: providers } }
  }),
  removeLlmProvider: (index) => set((s) => {
    if (!s.config) return {}
    const providers = s.config.llmProviders.filter((_, i) => i !== index)
    return { config: { ...s.config, llmProviders: providers } }
  }),
}))

export const useSettingsStore = store

if (typeof window !== 'undefined' && (import.meta as any).env?.DEV) {
  ;(window as any).__settingsStore = store
}
