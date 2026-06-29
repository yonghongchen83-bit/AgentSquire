import { expect } from '@wdio/globals'

const MOCK_CONFIG = {
  theme: 'dark',
  fontSize: 14,
  tabSize: 4,
  wordWrap: false,
  verboseLogging: false,
  llmProviders: [
    {
      providerType: 'openai',
      name: 'gpt-4o',
      apiKey: 'sk-test-key',
      model: 'gpt-4o',
      models: ['gpt-4o'],
      endpoint: 'https://api.openai.com/v1',
      category: 'openai',
    },
  ],
  searchExclude: ['node_modules'],
  terminalShell: '',
  terminalFontSize: 13,
}

const EMPTY_PROVIDER_CONFIG = {
  theme: 'dark',
  fontSize: 14,
  tabSize: 4,
  wordWrap: false,
  verboseLogging: false,
  llmProviders: [
    {
      providerType: 'openai',
      name: '',
      apiKey: '',
      model: '',
      models: [],
      endpoint: 'https://api.openai.com/v1',
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
    { timeout: 15000, timeoutMsg: 'App did not render within 15s' },
  )
}

describe('Task-007: Settings LLM Model Configuration Flow', () => {
  before(async () => {
    await waitForAppReady()
  })

  beforeEach(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(
      async () => $('#left-panel').isExisting(),
      { timeout: 10000 },
    )
  })

  it('should open settings to LLM tab when Model Configuration is clicked on welcome screen', async () => {
    const modelConfigBtn = await $('button=Model Configuration')
    await modelConfigBtn.waitForClickable()
    await modelConfigBtn.click()
    await browser.pause(500)

    const dialog = await $('[role="dialog"]')
    await expect(dialog).toBeDisplayed()

    const settingsTitle = await $('//*[text()="Settings"]')
    await expect(settingsTitle).toBeDisplayed()
  })

  it('should open settings to LLM tab when Test Connection is clicked on welcome screen', async () => {
    const testConnBtn = await $('button=Test Connection')
    await testConnBtn.waitForClickable()
    await testConnBtn.click()
    await browser.pause(500)

    const dialog = await $('[role="dialog"]')
    await expect(dialog).toBeDisplayed()
  })

  it('should keep settings open after interacting with provider dropdown', async () => {
    await browser.execute((config) => {
      const store = (window as any).__settingsStore
      if (store) {
        store.setState({ config, open: true, initialTab: 'llm' })
      }
    }, EMPTY_PROVIDER_CONFIG)
    await browser.pause(500)

    const dialog = await $('[role="dialog"]')
    await expect(dialog).toBeDisplayed()

    const llmTab = await $('//button[text()="LLM"]')
    await llmTab.click()
    await browser.pause(300)

    const providerSelect = await $('button=Select provider...')
    await providerSelect.waitForClickable()
    await providerSelect.click()
    await browser.pause(300)

    const dropdownItem = await $('//*[text()="ChatGPT"]')
    await expect(dropdownItem).toBeDisplayed()

    await browser.keys('\uE00C')
    await browser.pause(300)

    await expect(dialog).toBeDisplayed()
  })

  it('should keep settings open after interacting with Add model dropdown', async () => {
    await browser.execute((config) => {
      const store = (window as any).__settingsStore
      if (store) {
        store.setState({ config, open: true, initialTab: 'llm' })
      }
    }, MOCK_CONFIG)
    await browser.pause(500)

    const dialog = await $('[role="dialog"]')
    await expect(dialog).toBeDisplayed()

    const llmTab = await $('//button[text()="LLM"]')
    await llmTab.click()
    await browser.pause(300)

    const modelTag = await $('span=gpt-4o')
    await expect(modelTag).toBeDisplayed()

    const addModelSelect = await $('button=Add model...')
    await addModelSelect.waitForClickable()
    await addModelSelect.click()
    await browser.pause(300)

    const modelItem = await $('//*[text()="gpt-4o-mini"]')
    await expect(modelItem).toBeDisplayed()

    await browser.keys('\uE00C')
    await browser.pause(300)

    await expect(dialog).toBeDisplayed()
  })

  it('should display test connection button when provider has API key', async () => {
    await browser.execute((config) => {
      const store = (window as any).__settingsStore
      if (store) {
        store.setState({ config, open: true, initialTab: 'llm' })
      }
    }, MOCK_CONFIG)
    await browser.pause(500)

    const llmTab = await $('//button[text()="LLM"]')
    await llmTab.click()
    await browser.pause(300)

    const testBtn = await $('button=Test')
    await expect(testBtn).toBeDisplayed()
    await expect(testBtn).toBeEnabled()
  })

  it('should invoke test connection and show result', async () => {
    await browser.execute(() => {
      const store = (window as any).__settingsStore
      if (store) {
        store.setState({
          config: {
            theme: 'dark', fontSize: 14, tabSize: 4, wordWrap: false, verboseLogging: false,
            llmProviders: [
              { providerType: 'openai', name: 'OpenCode Zen Free', apiKey: '__test_key__', model: 'gpt-5-nano', models: ['gpt-5-nano'], endpoint: 'https://opencode.ai/zen/v1', category: 'opencode-zen-free' },
            ],
            searchExclude: [], terminalShell: '', terminalFontSize: 13,
          },
          open: true, initialTab: 'llm',
        })
      }
    })
    await browser.pause(500)

    const llmTab = await $('//button[text()="LLM"]')
    await llmTab.click()
    await browser.pause(300)

    const testBtn = await $('button=Test')
    await testBtn.waitForClickable()
    await testBtn.click()

    await browser.pause(2000)

    const resultMsg = await $('[class*="text-green"],[class*="text-red"]')
    await expect(resultMsg).toBeDisplayed()
  })

  it('should test connection with OpenCode Zen Free using real API key', async () => {
    const apiKey = 'sk-xsjXJidLxkJxBtkhPpugyqnxNC1maFiIKuMnQGkRgKExOd3s7uWbwWJiebO0xAvs'
    await browser.execute((key) => {
      const store = (window as any).__settingsStore
      if (store) {
        store.setState({
          config: {
            theme: 'dark', fontSize: 14, tabSize: 4, wordWrap: false, verboseLogging: false,
            llmProviders: [
              { providerType: 'openai', name: 'OpenCode Zen Free', apiKey: key, model: 'deepseek-v4-flash-free', models: ['deepseek-v4-flash-free'], endpoint: 'https://opencode.ai/zen/v1', category: 'opencode-zen-free' },
            ],
            searchExclude: [], terminalShell: '', terminalFontSize: 13,
          },
          open: true, initialTab: 'llm',
        })
      }
    }, apiKey)
    await browser.pause(500)

    const llmTab = await $('//button[text()="LLM"]')
    await llmTab.click()
    await browser.pause(300)

    const testBtn = await $('button=Test')
    await testBtn.waitForClickable()
    await testBtn.click()

    await browser.pause(5000)

    const resultMsg = await $('[class*="text-green"],[class*="text-red"]')
    await expect(resultMsg).toBeDisplayed()

    const resultText = await resultMsg.getText()
    expect(resultText).toContain('Connection successful')
  })
})
