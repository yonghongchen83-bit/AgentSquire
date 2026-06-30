import { describe, it, expect, beforeEach } from 'vitest'
import { useSettingsStore } from './settings-store'

describe('settings-store', () => {
  beforeEach(() => {
    useSettingsStore.setState({ open: false, config: null, showSplash: true })
  })

  it('starts closed with no config', () => {
    const s = useSettingsStore.getState()
    expect(s.open).toBe(false)
    expect(s.config).toBeNull()
    expect(s.showSplash).toBe(true)
  })

  it('setOpen toggles dialog', () => {
    useSettingsStore.getState().setOpen(true)
    expect(useSettingsStore.getState().open).toBe(true)
  })

  it('setConfig stores config', () => {
    const config = {
      theme: 'dark' as const,
      fontSize: 16,
      tabSize: 4,
      wordWrap: true,
      llmProviders: [],
      mcpServers: [],
      searchExclude: ['node_modules'],
      terminalShell: '/bin/zsh',
      terminalFontSize: 14,
      verboseLogging: false,
    }
    useSettingsStore.getState().setConfig(config)
    expect(useSettingsStore.getState().config).toEqual(config)
  })

  it('setShowSplash hides splash', () => {
    useSettingsStore.getState().setShowSplash(false)
    expect(useSettingsStore.getState().showSplash).toBe(false)
  })

  it('updateVerboseLogging toggles verbose logging', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateVerboseLogging(true)
    expect(useSettingsStore.getState().config?.verboseLogging).toBe(true)
  })

  it('updateTheme changes theme', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateTheme('dark')
    expect(useSettingsStore.getState().config?.theme).toBe('dark')
  })

  it('updateEditorFontSize changes font size', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 13, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateEditorFontSize(18)
    expect(useSettingsStore.getState().config?.fontSize).toBe(18)
  })

  it('updateEditorWordWrap toggles word wrap', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateEditorWordWrap(true)
    expect(useSettingsStore.getState().config?.wordWrap).toBe(true)
  })

  it('updateEditorTabSize changes tab size', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateEditorTabSize(2)
    expect(useSettingsStore.getState().config?.tabSize).toBe(2)
  })

  it('updateTerminalFontSize changes terminal font size', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateTerminalFontSize(16)
    expect(useSettingsStore.getState().config?.terminalFontSize).toBe(16)
  })

  it('updateTerminalShell changes shell path', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateTerminalShell('/bin/bash')
    expect(useSettingsStore.getState().config?.terminalShell).toBe('/bin/bash')
  })

  it('updateSearchExclude changes exclude patterns', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: ['node_modules'], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateSearchExclude(['.git', 'dist'])
    expect(useSettingsStore.getState().config?.searchExclude).toEqual(['.git', 'dist'])
  })

  it('addLlmProvider adds empty provider', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [], mcpServers: [], searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().addLlmProvider()
    expect(useSettingsStore.getState().config?.llmProviders).toHaveLength(1)
    expect(useSettingsStore.getState().config?.llmProviders[0].name).toBe('')
  })

  it('removeLlmProvider removes by index', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [{ providerType: 'openai', name: 'A', apiKey: '', model: '', models: [], endpoint: '' }],
      mcpServers: [],
      searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().removeLlmProvider(0)
    expect(useSettingsStore.getState().config?.llmProviders).toHaveLength(0)
  })

  it('updateLlmProvider updates specific provider field', () => {
    useSettingsStore.getState().setConfig({
      theme: 'light', fontSize: 14, tabSize: 4, wordWrap: false,
      llmProviders: [{ providerType: 'openai', name: 'A', apiKey: '', model: 'gpt-3', models: ['gpt-3'], endpoint: '' }],
      mcpServers: [],
      searchExclude: [], terminalShell: '', terminalFontSize: 13,
      verboseLogging: false,
    })
    useSettingsStore.getState().updateLlmProvider(0, { model: 'gpt-4' })
    expect(useSettingsStore.getState().config?.llmProviders[0].model).toBe('gpt-4')
  })
})
