import { expect } from '@wdio/globals'

describe('Task-010: Reuse existing session and repro stuck path', () => {
  beforeEach(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })
  })

  it('reuses latest existing session with OpenCode Zen Free deepseek and does not hit no-finish fallback', async () => {
    const providerName = 'OpenCode Zen Free'
    const modelName = 'deepseek-v4-flash-free'

    await browser.execute((provider: string, model: string) => {
      window.localStorage.setItem('chat:last-model-selection', JSON.stringify({ provider, model }))
    }, providerName, modelName)

    await browser.refresh()
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })

    // Reuse existing session: load all conversations and select the latest one.
    const selected = await browser.executeAsync((done) => {
      const cs = (window as any).__chatStore
      if (!cs) return done({ ok: false, reason: 'chat store missing' })

      cs.getState().loadConversations()
        .then(() => {
          const conversations = cs.getState().conversations
          if (!conversations || conversations.length === 0) {
            done({ ok: false, reason: 'no existing conversations' })
            return
          }
          const latest = conversations[0]
          cs.getState().selectConversation(latest.id)
            .then(() => done({ ok: true, id: latest.id }))
            .catch((e: unknown) => done({ ok: false, reason: String(e) }))
        })
        .catch((e: unknown) => done({ ok: false, reason: String(e) }))
    }) as { ok: boolean; reason?: string; id?: string }

    expect(selected.ok).toBe(true)

    const prompt = [
      'Redo the previous task in this same session.',
      'First check current directory/resources, then produce the WiFi comparison HTML page.',
    ].join(' ')

    const input = await $('textarea[placeholder="Ask anything..."]')
    await input.waitForDisplayed({ timeout: 10000 })
    await input.setValue(prompt)
    await browser.keys(['Control', 'Enter'])

    await browser.pause(18000)

    const pageText = await $('body').getText()
    expect(pageText.includes('Stream ended without finish reason and no usable output')).toBe(false)

    // Either live status or assistant output must be visible.
    const hasLiveProgress =
      pageText.includes('Starting generation...') ||
      pageText.includes('Preparing tools...') ||
      pageText.includes('Contacting model...') ||
      pageText.includes('Invoking tool') ||
      pageText.includes('Waiting for approval') ||
      pageText.includes('Tool ') ||
      pageText.includes('Completed')
    const hasAssistantOutput = pageText.includes('Assistant')
    expect(hasLiveProgress || hasAssistantOutput).toBe(true)
  })
})
