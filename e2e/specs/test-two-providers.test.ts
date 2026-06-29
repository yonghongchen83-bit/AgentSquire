import { expect } from '@wdio/globals'

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

describe('Two Providers in Chat — Normal User Flow', () => {
  before(async () => { await waitForAppReady() })

  beforeEach(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(
      async () => $('#left-panel').isExisting(),
      { timeout: 10000 },
    )
  })

  it('add 2nd provider via settings, save closes panel, then chat shows both', async () => {
    // ── Step 1: Open settings via "Model Configuration" ──
    const modelConfigBtn = await $('button=Model Configuration')
    await modelConfigBtn.waitForClickable()
    await modelConfigBtn.click()
    await browser.pause(1000)

    // ── Step 2: Click "Add Provider" ──
    const addProviderBtn = await $('button=Add Provider')
    await addProviderBtn.waitForClickable()
    await addProviderBtn.click()
    await browser.pause(500)

    // ── Step 3: Click the new provider's "Select provider..." dropdown ──
    // The first provider shows "DeepSeek" (already configured).
    // The new, empty provider shows "Select provider..."
    // Use click via browser.execute to bypass Radix overlay interception
    await browser.execute(() => {
      const triggers = document.querySelectorAll<HTMLElement>('[role="combobox"]')
      for (const t of triggers) {
        if (t.textContent?.includes('Select provider')) {
          t.click()
          return
        }
      }
    })
    await browser.pause(500)

    // ── Step 4: Select "OpenCode Zen" ──
    const opencodeOption = await $('//*[text()="OpenCode Zen"]')
    await opencodeOption.waitForDisplayed()
    await opencodeOption.click()
    await browser.pause(500)

    // ── Step 5: Save — this closes the settings panel ──
    const saveBtn = await $('button=Save')
    await saveBtn.waitForClickable()
    await saveBtn.click()
    await browser.pause(800)

    // Verify dialog is closed
    const dialog = await $('[role="dialog"]')
    await expect(dialog).not.toBeDisplayed()

    // ── Step 6: Read chat store state directly to check providers ──
    const chatState: any = await browser.execute(() => {
      const cs = (window as any).__chatStore
      if (!cs) return null
      const s = cs.getState()
      return {
        providers: s.providers,
        selectedProvider: s.selectedProvider,
        selectedModel: s.selectedModel,
      }
    })
    console.log('CHAT STORE STATE:', JSON.stringify(chatState, null, 2))

    expect(chatState).not.toBeNull()
    expect(chatState.providers.length).toBe(2)
    expect(chatState.providers[0].name).toBe('DeepSeek')
    expect(chatState.providers[1].name).toBe('OpenCode Zen')
    expect(chatState.providers[0].models).toContain('deepseek-v4-flash')
    expect(chatState.providers[1].models).toContain('gpt-5-nano')
  })
})
