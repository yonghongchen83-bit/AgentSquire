import { expect } from '@wdio/globals'

describe('Task-009: Stuck execution visibility and recovery', () => {
  beforeEach(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })
  })

  it('uses OpenCode Zen Free + deepseek-v4-flash-free and shows live status/abort', async () => {
    const providerName = 'OpenCode Zen Free'
    const modelName = 'deepseek-v4-flash-free'

    const providers = await browser.execute(async () => {
      return (window as any).__TAURI_INTERNALS__.invoke('list_providers')
    }) as Array<{ name: string; models: string[] }>

    const target = providers.find((p) => p.name === providerName)
    expect(!!target).toBe(true)
    expect(target!.models.includes(modelName)).toBe(true)

    await browser.execute((provider: string, model: string) => {
      window.localStorage.setItem('chat:last-model-selection', JSON.stringify({ provider, model }))
    }, providerName, modelName)

    await browser.refresh()
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })

    const prompt = [
      'Create a really nice HTML comparison page for WiFi models.',
      'First check what you can access, look at current directory, then build something useful.',
    ].join(' ')

    const input = await $('textarea[placeholder="Ask anything..."]')
    await input.waitForDisplayed({ timeout: 10000 })
    await input.setValue(prompt)

    await browser.keys(['Control', 'Enter'])

    await browser.pause(16000)

    const statusCandidates = [
      'Starting generation...',
      'Preparing tools...',
      'Contacting model...',
      'Invoking tool',
      'Waiting for approval',
      'Tool ',
      'Completed',
      'Model request failed',
      'Model stream error',
      'LLM returned an error',
      'Authentication failed',
    ]

    const pageText = await $('body').getText()
    const hasStatus = statusCandidates.some((s) => pageText.includes(s))
    const hasFallbackError = pageText.includes('Stream ended without finish reason and no usable output')
    const hasAssistantContent = pageText.includes('Assistant')

    expect(hasFallbackError).toBe(false)
    expect(hasStatus || hasAssistantContent).toBe(true)

    const approvalButton = await $('button=Approve')
    if (await approvalButton.isExisting()) {
      await approvalButton.click()
      await browser.pause(3000)
    }

    const stopButton = await $('button=Stop')
    if (await stopButton.isExisting()) {
      await stopButton.click()
      await browser.waitUntil(async () => !(await $('button=Stop').isExisting()), {
        timeout: 10000,
        timeoutMsg: 'Stop button should disappear after manual abort',
      })
    }
  })
})
