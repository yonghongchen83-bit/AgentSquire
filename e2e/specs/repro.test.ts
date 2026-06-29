import { expect } from '@wdio/globals'

describe('Reproduce: 2 providers in chat', () => {
  before(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })
  })

  beforeEach(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 10000 })
  })

  it('settings with 1 provider -> add 2nd -> save -> chat shows both', async () => {
    // Step 1: Verify there is 1 provider in config (DeepSeek from config file)
    const providers1 = await browser.execute(() =>
      window.__TAURI_INTERNALS__.invoke('list_providers')
    )
    console.log('INITIAL providers:', JSON.stringify(providers1))
    expect(Array.isArray(providers1)).toBe(true)

    // Step 2: Open settings dialog
    const modelConfigBtn = await $('button=Model Configuration')
    await modelConfigBtn.waitForClickable()
    await modelConfigBtn.click()
    await browser.pause(500)

    // Check dialog is open
    let dialog = await $('[role="dialog"]')
    await expect(dialog).toBeDisplayed()

    // Step 3: Click "Add Provider"
    const addBtn = await $('button=Add Provider')
    await addBtn.waitForClickable()
    await addBtn.click()
    await browser.pause(500)

    // Step 4: Click the NEW "Select provider..." (not the existing provider's name)
    const selects = await $$('[role="combobox"]')
    console.log('comboboxes found:', selects.length)
    for (const s of selects) {
      const text = await s.getText()
      console.log(' - combobox text:', JSON.stringify(text))
    }

    // The new (second) provider entry should have "Select provider..." text
    if (selects.length > 1) {
      await selects[1].click()
      await browser.pause(300)

      // Select "OpenCode Zen"
      const item = await $('//*[text()="OpenCode Zen"]')
      await item.waitForDisplayed()
      await item.click()
      await browser.pause(300)
    }

    // Step 5: Click Save
    const saveBtn = await $('button=Save')
    await saveBtn.waitForClickable()
    await saveBtn.click()
    await browser.pause(1000)

    // Verify dialog closed
    dialog = await $('[role="dialog"]')
    await expect(dialog).not.toBeDisplayed()

    // Step 6: Query providers from chat store
    const providers2 = await browser.execute(() =>
      window.__TAURI_INTERNALS__.invoke('list_providers')
    )
    console.log('AFTER SAVE providers:', JSON.stringify(providers2))

    // Step 7: Read chat store state
    const chatState = await browser.execute(() => {
      const cs = (window as any).__chatStore
      if (!cs) return null
      const s = cs.getState()
      return { providers: s.providers, selectedProvider: s.selectedProvider, selectedModel: s.selectedModel }
    })
    console.log('CHAT STORE:', JSON.stringify(chatState))
  })
})
