import { expect } from '@wdio/globals'

const TWO_PROVIDERS_CONFIG = {
  theme: 'dark',
  fontSize: 14,
  tabSize: 4,
  wordWrap: false,
  verboseLogging: false,
  llmProviders: [
    {
      providerType: 'openai',
      name: 'DeepSeek',
      apiKey: 'sk-deepseek-test',
      model: 'deepseek-v4-flash',
      models: ['deepseek-v4-flash', 'deepseek-chat'],
      endpoint: 'https://api.deepseek.com',
      category: 'deepseek',
    },
    {
      providerType: 'openai',
      name: 'OpenCode Zen',
      apiKey: 'sk-opencode-test',
      model: 'gpt-5-nano',
      models: ['gpt-5-nano', 'qwen3.6-plus'],
      endpoint: 'https://opencode.ai/zen/v1/chat/completions',
      category: 'opencode-zen',
    },
  ],
  searchExclude: ['node_modules'],
  terminalShell: '',
  terminalFontSize: 13,
}

async function waitForAppReady(): Promise<void> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => {
      const exists = await $('#left-panel').isExisting()
      return exists
    },
    { timeout: 15000 },
  )
}

describe('Debug Providers', () => {
  before(async () => { await waitForAppReady() })

  it('check list_providers returns both after save', async () => {
    await browser.execute((cfg) => {
      return window.__TAURI_INTERNALS__.invoke('save_config', { newConfig: cfg })
    }, TWO_PROVIDERS_CONFIG)
    await browser.pause(500)

    // Query list_providers directly
    const providers = await browser.execute(() => {
      return window.__TAURI_INTERNALS__.invoke('list_providers')
    })
    console.log('list_providers result:', JSON.stringify(providers, null, 2))
    expect(Array.isArray(providers)).toBe(true)
    expect(providers.length).toBe(2)
  })

  it('check chat store providers after save + providers-changed event', async () => {
    // Clear chat store's providers
    await browser.execute(() => {
      const cs = (window as any).__chatStore
      if (cs) cs.setState({ providers: [], selectedProvider: '', selectedModel: '' })
    })
    await browser.pause(200)

    // First save the config with 2 providers via the normal settings dialog flow
    // (which emits 'providers-changed' automatically)
    await browser.execute((cfg) => {
      return window.__TAURI_INTERNALS__.invoke('save_config', { newConfig: cfg })
    }, TWO_PROVIDERS_CONFIG)
    await browser.pause(300)

    // Manually trigger the providers-changed event that the settings dialog would emit
    // We need to use Tauri's event system
    await browser.execute(() => {
      // The chat panel listens for 'providers-changed' via listen()
      // We can trigger it by emitting from the Rust side... but we can't do that.
      // Instead, let's directly call loadProviders on the chat store
      const cs = (window as any).__chatStore
      if (cs) cs.getState().loadProviders()
    })
    await browser.pause(1000)

    // Check the chat store's providers state
    const state: any = await browser.execute(() => {
      const cs = (window as any).__chatStore
      if (!cs) return null
      const s = cs.getState()
      return {
        providers: s.providers,
        selectedProvider: s.selectedProvider,
        selectedModel: s.selectedModel,
      }
    })
    console.log('CHAT STORE STATE:', JSON.stringify(state, null, 2))
    expect(state).not.toBeNull()
    expect(state.providers.length).toBe(2)
    expect(state.providers[0].name).toBe('DeepSeek')
    expect(state.providers[1].name).toBe('OpenCode Zen')
  })
})
