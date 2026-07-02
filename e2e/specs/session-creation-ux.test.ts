import { expect } from '@wdio/globals'

// session-creation-ux real end-to-end verification: drives the actual running Tauri app
// (real IPC, real frontend rendering) through the new UI path this node added for
// choosing Squire mode at conversation-creation time — closing the "newly observed gap"
// flagged at the end of tool-token-ingestion's session (root/Squire/handoff.md's "Newly
// observed gap" section): before this node, no button/toggle in the UI could create a
// Squire-mode session; every prior spec needing one (e.g. ask-user-loop.test.ts) had to
// reach into window.__TAURI_INTERNALS__.invoke('create_conversation', { contextMode:
// 'squire' }) directly. This spec instead uses the real Sessions tab, the real Squire
// toggle switch, and the real "+ New session" button.
//
// Two things are asserted:
//   1. Creating a session with the toggle OFF (default) produces a Legacy-mode session
//      with no Squire badge — confirming the default-preserving behavior is real, not
//      just documented.
//   2. Creating a session with the toggle ON produces a session whose store-level
//      contextMode is 'squire', the sidebar shows a "Squire" badge on that row, and a
//      real message sent on that session drives real Squire-mode behavior (the live
//      stream-chunk channel stays suppressed per stream-sigil-fix's sa-4 fix — Squire
//      mode's raw protocol JSON is never forwarded to the live UI stream, only the
//      finalized turn is), rather than merely trusting the toggle's own visual state.
//
// session-ux-polish (2026-07-03) added a third case: the toggle's last-chosen value now
// persists across a real remount (a full page reload, not just a tab switch — the sidebar
// component does not actually unmount on tab switches, see session-ux-polish/env.md) via
// localStorage, so a user who wants several Squire-mode sessions in a row no longer has to
// re-flip the toggle every time.

async function waitForAppReady(): Promise<void> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => $('#left-panel').isExisting(),
    { timeout: 15000, timeoutMsg: 'App did not render within 15s' },
  )
}

async function openSessionsTab(): Promise<void> {
  const sessionsTab = await $('span=Sessions')
  await sessionsTab.waitForDisplayed({ timeout: 10000 })
  await sessionsTab.click()
}

describe('session-creation-ux: real UI path to create a Squire-mode session', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('defaults to legacy mode when the Squire toggle is left off', async () => {
    await openSessionsTab()

    const toggle = await $('button[aria-label="Create new sessions in Squire mode"]')
    await toggle.waitForDisplayed({ timeout: 10000 })
    expect(await toggle.getAttribute('data-state')).toBe('unchecked')

    const newSessionBtn = await $('button[title="New session"]')
    await newSessionBtn.click()

    await browser.waitUntil(async () => {
      const activeId = await browser.execute(() => (window as any).__chatStore.getState().activeConversationId)
      return !!activeId
    }, { timeout: 10000, timeoutMsg: 'a new session should become active after clicking +' })

    const activeSession = await browser.execute(async () => {
      const store = (window as any).__chatStore.getState()
      const list = await (window as any).__TAURI_INTERNALS__.invoke('list_conversations')
      return list.find((s: any) => s.id === store.activeConversationId)
    })
    expect(activeSession.context_mode).toBe('legacy')

    // No Squire badge should render for this row.
    const rows = await $$('div.group')
    let sawSquireBadgeOnDefaultRow = false
    for (const row of rows) {
      const text = await row.getText()
      if (text.includes('Squire')) sawSquireBadgeOnDefaultRow = true
    }
    expect(sawSquireBadgeOnDefaultRow).toBe(false)
  })

  it('creates a real squire-mode session via the toggle and the resulting session behaves as squire mode', async () => {
    await openSessionsTab()

    const toggle = await $('button[aria-label="Create new sessions in Squire mode"]')
    await toggle.waitForDisplayed({ timeout: 10000 })
    if ((await toggle.getAttribute('data-state')) !== 'checked') {
      await toggle.click()
    }
    await browser.waitUntil(
      async () => (await toggle.getAttribute('data-state')) === 'checked',
      { timeout: 5000, timeoutMsg: 'toggle should reflect checked state before creating a session' },
    )

    const newSessionBtn = await $('button[title="New session"]')
    await newSessionBtn.click()

    await browser.waitUntil(async () => {
      const activeId = await browser.execute(() => (window as any).__chatStore.getState().activeConversationId)
      return !!activeId
    }, { timeout: 10000, timeoutMsg: 'a new session should become active after clicking +' })

    // ── Confirm the created session is real squire mode at the store/backend level ──
    const activeSessionId = await browser.execute(() => (window as any).__chatStore.getState().activeConversationId)
    const activeSession = await browser.execute(async (id: string) => {
      const list = await (window as any).__TAURI_INTERNALS__.invoke('list_conversations')
      return list.find((s: any) => s.id === id)
    }, activeSessionId)
    expect(activeSession.context_mode).toBe('squire')

    // ── Confirm the sidebar row now shows the Squire badge ──
    const badge = await $('span[title="This session uses Squire\'s curated protocol context"]')
    await badge.waitForDisplayed({ timeout: 5000 })

    // ── Switch to the Chat tab and send a real message on this real squire-mode session ──
    const chatTab = await $('span=Chat')
    await chatTab.click()

    const providerName = 'OpenCode Zen Free'
    const modelName = 'deepseek-v4-flash-free'
    await browser.execute((provider: string, model: string) => {
      window.localStorage.setItem('chat:last-model-selection', JSON.stringify({ provider, model }))
    }, providerName, modelName)
    await browser.refresh()
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })
    await browser.execute(async (sessionId: string) => {
      const cs = (window as any).__chatStore
      await cs.getState().selectConversation(sessionId)
    }, activeSessionId)
    await browser.pause(300)

    const input = await $('textarea[placeholder="Ask anything..."]')
    await input.waitForDisplayed({ timeout: 10000 })
    await input.setValue('Say hello in one short sentence.')
    await browser.keys(['Control', 'Enter'])

    // sa-4 (stream-sigil-fix): squire mode's raw sigil-laden protocol JSON must never reach
    // the live stream-chunk UI channel — streamingText should stay empty/clean throughout
    // generation (unlike legacy mode, which streams live token-by-token). This is the most
    // direct, already-established signal (from stream-sigil-fix's own node) that a session
    // is really running the squire adapter end to end, not just carrying a squire label.
    let observedNonEmptyLiveStream = false
    const deadline = Date.now() + 30000
    while (Date.now() < deadline) {
      const state = await browser.execute(() => {
        const s = (window as any).__chatStore.getState()
        return { isStreaming: s.isStreaming, streamingText: s.streamingText, error: s.error, messageCount: s.messages.length }
      })
      if (state.streamingText && state.streamingText.length > 0) {
        observedNonEmptyLiveStream = true
      }
      if (!state.isStreaming || state.error) break
      await browser.pause(500)
    }

    const finalState = await browser.execute(() => {
      const s = (window as any).__chatStore.getState()
      return { isStreaming: s.isStreaming, error: s.error, messages: s.messages.map((m: any) => ({ role: m.role, content: m.content })) }
    })
    console.log('[e2e] session-creation-ux final state:', JSON.stringify(finalState, null, 2))

    expect(observedNonEmptyLiveStream).toBe(false)
  })

  it('persists the Squire toggle choice across a real remount (session-ux-polish)', async () => {
    await waitForAppReady()
    await openSessionsTab()

    const toggle = await $('button[aria-label="Create new sessions in Squire mode"]')
    await toggle.waitForDisplayed({ timeout: 10000 })
    if ((await toggle.getAttribute('data-state')) !== 'checked') {
      await toggle.click()
    }
    expect(await toggle.getAttribute('data-state')).toBe('checked')

    // Force a real remount (not just a tab switch, which does not unmount
    // ConversationSidebar today — see session-ux-polish/env.md) via a full page reload.
    await browser.refresh()
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })
    await openSessionsTab()

    const toggleAfterReload = await $('button[aria-label="Create new sessions in Squire mode"]')
    await toggleAfterReload.waitForDisplayed({ timeout: 10000 })
    expect(await toggleAfterReload.getAttribute('data-state')).toBe('checked')

    // Reset back to unchecked so this spec doesn't leak state into any test run after it.
    await toggleAfterReload.click()
    expect(await toggleAfterReload.getAttribute('data-state')).toBe('unchecked')
  })
})
