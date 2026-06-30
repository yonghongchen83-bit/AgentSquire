import { expect } from '@wdio/globals'

describe('Task-011: History roles visibility', () => {
  it('loads latest persisted session and renders both user and assistant messages when present', async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })

    const result = await browser.executeAsync((done) => {
      const cs = (window as any).__chatStore
      if (!cs) return done({ ok: false, reason: 'chat store missing' })

      cs.getState().loadConversations()
        .then(() => {
          const conversations = cs.getState().conversations
          if (!conversations?.length) {
            done({ ok: false, reason: 'no conversations' })
            return
          }
          const latest = conversations[0]
          cs.getState().selectConversation(latest.id)
            .then(() => {
              const messages = cs.getState().messages
              done({
                ok: true,
                sessionId: latest.id,
                title: latest.title,
                roles: messages.map((m: { role: string }) => m.role),
                count: messages.length,
              })
            })
            .catch((e: unknown) => done({ ok: false, reason: String(e) }))
        })
        .catch((e: unknown) => done({ ok: false, reason: String(e) }))
    }) as { ok: boolean; reason?: string; sessionId?: string; title?: string; roles?: string[]; count?: number }

    expect(result.ok).toBe(true)
    expect((result.roles ?? []).includes('user')).toBe(true)
    expect((result.roles ?? []).includes('assistant')).toBe(true)

    await browser.pause(500)
    const pageText = await $('body').getText()
    expect(pageText.includes('You')).toBe(true)
    expect(pageText.includes('Assistant')).toBe(true)
  })
})
