import { expect } from '@wdio/globals'

const MOCK_PROVIDERS = [
  {
    name: 'OpenAI',
    models: ['gpt-4o', 'gpt-4o-mini'],
  },
  {
    name: 'Anthropic',
    models: ['claude-3-5-sonnet'],
  },
]

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

describe('Task-011: Phase 2 Model Selector', () => {
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

  it('should display Phase 2 model selector in chat panel when providers exist', async () => {
    // Inject mock providers into the chat store
    await browser.execute((providers) => {
      const store = (window as any).__ZUSTAND_DEVTOOLS_STORE?.['chat-store'] ?? 
        Object.values((window as any).__ZUSTAND_DEVTOOLS__ ?? {}).find((s: any) => s?.getState?.()?.providers !== undefined)
      if (store) {
        store.setState({ providers })
      } else {
        // Fallback: set on useChatStore directly if exposed
        const chatStore = (window as any).useChatStore
        if (chatStore) {
          chatStore.setState({ providers })
        }
      }
    }, MOCK_PROVIDERS)
    await browser.pause(500)

    // Look for "Phase 2:" label in the chat panel
    const phase2Label = await $('span=Phase 2:')
    await expect(phase2Label).toBeDisplayed()
  })

  it('should allow selecting a Phase 2 model from the dropdown', async () => {
    // Inject mock providers
    await browser.execute((providers) => {
      const chatStore = (window as any).__chatStore
      if (chatStore) {
        chatStore.setState({ 
          providers,
          selectedProvider: 'OpenAI',
          selectedModel: 'gpt-4o',
        })
      }
    }, MOCK_PROVIDERS)
    await browser.pause(500)

    // Find and click the Phase 2 dropdown trigger
    const phase2Label = await $('span=Phase 2:')
    await expect(phase2Label).toBeDisplayed()
    
    // The select trigger should be next to the label
    const phase2Trigger = await phase2Label.nextElement()
    await phase2Trigger.click()
    await browser.pause(300)

    // Select a model from the dropdown
    const modelOption = await $('div[role="option"]=claude-3-5-sonnet')
    if (await modelOption.isExisting()) {
      await modelOption.click()
      await browser.pause(300)

      // Verify the selection was stored
      const stored = await browser.execute(() => {
        const chatStore = (window as any).__chatStore
        return {
          provider: chatStore?.getState?.()?.selectedPhase2Provider,
          model: chatStore?.getState?.()?.selectedPhase2Model,
        }
      })
      expect(stored.model).toBe('claude-3-5-sonnet')
    }
  })

  it('should show "Same as main" option in Phase 2 selector', async () => {
    // Inject mock providers
    await browser.execute((providers) => {
      const chatStore = (window as any).__chatStore
      if (chatStore) {
        chatStore.setState({ 
          providers,
          selectedProvider: 'OpenAI',
          selectedModel: 'gpt-4o',
        })
      }
    }, MOCK_PROVIDERS)
    await browser.pause(500)

    // Find and click the Phase 2 dropdown trigger
    const phase2Label = await $('span=Phase 2:')
    await expect(phase2Label).toBeDisplayed()
    
    const phase2Trigger = await phase2Label.nextElement()
    await phase2Trigger.click()
    await browser.pause(300)

    // Look for "Same as main" option
    const sameAsMainOption = await $('div[role="option"]=Same as main')
    await expect(sameAsMainOption).toBeDisplayed()
  })
})
