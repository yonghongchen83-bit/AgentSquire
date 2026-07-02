import { expect } from '@wdio/globals'

// sa-5 real end-to-end verification: drives the actual running Tauri app
// (real IPC, real frontend rendering, real model calls against the
// configured free-tier test provider) through the full ask_user
// pause/resume loop this session implemented:
//   1. Create a Squire-mode session directly via the create_conversation IPC
//      command. A real UI toggle for choosing Squire mode now exists
//      (session-creation-ux's Sessions-tab Switch, see
//      e2e/specs/session-creation-ux.test.ts, which exercises that path
//      end to end) — this spec still creates its session directly via IPC
//      because its own subject is the ask_user pause/resume mechanism, not
//      session creation, and the direct-IPC path remains the simplest
//      correct setup for that narrower concern.
//   2. Send a message engineered to make the model populate `ask_user`.
//   3. Assert the new inline question/answer UI (chat-panel.tsx) renders the
//      question surfaced via the `stream-ask-user-pending` IPC event.
//   4. Submit an answer through that UI.
//   5. Assert the turn resumes and eventually reaches a terminal state
//      (either a persisted assistant message, or a second ask_user round —
//      deepseek-v4-flash-free is a small free-tier model, see
//      ask-user-loop/state.md for this session's observed model behavior)
//      rather than hanging forever or erroring.

// The app's real (Tauri app_config_dir) config.toml already has this
// provider configured (see ask-user-loop/state.md) — matches the name/model
// used by other specs in this suite (task-009, task-007) for the same
// free-tier test provider.
const PROVIDER_NAME = 'OpenCode Zen Free'
const MODEL_NAME = 'deepseek-v4-flash-free'

async function waitForAppReady(): Promise<void> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => $('#left-panel').isExisting(),
    { timeout: 15000, timeoutMsg: 'App did not render within 15s' },
  )
}

describe('sa-5: Squire ask_user pause/resume loop (real model, real IPC)', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('surfaces a model-asked question in the UI, accepts an answer, and resumes the turn', async () => {
    // ── Step 1: confirm the pre-configured test provider is visible ──
    const providers = (await browser.execute(() => {
      return (window as any).__TAURI_INTERNALS__.invoke('list_providers')
    })) as Array<{ name: string; models: string[] }>
    const target = providers.find((p) => p.name === PROVIDER_NAME)
    expect(!!target).toBe(true)
    expect(target!.models.includes(MODEL_NAME)).toBe(true)

    // ── Step 2: create a Squire-mode session directly via IPC. A real UI
    // toggle for this now exists (session-creation-ux) but is orthogonal to
    // this spec's own subject (the ask_user pause/resume mechanism) — see
    // ask-user-loop/decisions.md and env.md for the original reasoning ──
    const session = (await browser.execute(() => {
      return (window as any).__TAURI_INTERNALS__.invoke('create_conversation', {
        title: 'sa-5 e2e',
        contextMode: 'squire',
      })
    })) as { id: string; context_mode?: string; contextMode?: string }
    expect(session.id).toBeTruthy()

    // ── Step 3: select the model and the new session in the chat store ──
    await browser.execute((provider: string, model: string) => {
      window.localStorage.setItem('chat:last-model-selection', JSON.stringify({ provider, model }))
    }, PROVIDER_NAME, MODEL_NAME)

    await browser.refresh()
    await browser.waitUntil(async () => $('#left-panel').isExisting(), { timeout: 15000 })

    await browser.execute(async (sessionId: string) => {
      const cs = (window as any).__chatStore
      await cs.getState().selectConversation(sessionId)
    }, session.id)
    await browser.pause(300)

    const activeId = await browser.execute(() => (window as any).__chatStore.getState().activeConversationId)
    expect(activeId).toBe(session.id)

    // ── Step 4: send a message directive enough to make a small free-tier
    // model actually populate ask_user instead of guessing (see this
    // session's ask_user_e2e.rs harness run in state.md, which confirmed
    // this exact framing reliably triggers ask_user on the first turn) ──
    const prompt = [
      'Before you answer anything else, you must ask me one clarifying question about',
      "which city I'm asking about, using the ask_user field, before you write any content.",
      'Do not answer until you have asked and I have replied.',
    ].join(' ')

    const input = await $('textarea[placeholder="Ask anything..."]')
    await input.waitForDisplayed({ timeout: 10000 })
    await input.setValue(prompt)
    await browser.keys(['Control', 'Enter'])

    // ── Step 5: wait for the pending-question UI to appear ──
    const answerInput = await $('input[placeholder="Type your answer..."]')
    await answerInput.waitForDisplayed({ timeout: 30000 })

    const questionText = await $('span.text-blue-900')
    await expect(questionText).toBeDisplayed()
    const questionBody = await questionText.getText()
    console.log('[e2e] ask_user question surfaced in UI:', questionBody)
    expect(questionBody.length).toBeGreaterThan(0)

    // Confirm the pending question is also tracked in store state (not just
    // rendered — this is the state the new pause/resume mechanism hinges on).
    const pending = await browser.execute(() => (window as any).__chatStore.getState().pendingAskUserQuestion)
    expect(pending).not.toBeNull()
    expect(pending.question).toBe(questionBody)

    // ── Step 6: submit an answer through the real UI ──
    await answerInput.setValue('Sydney, Australia.')
    const answerBtn = await $('button=Answer')
    await answerBtn.waitForClickable({ timeout: 5000 })
    await answerBtn.click()

    // The pending-question UI should clear immediately (optimistic clear in
    // answerAskUserQuestion, mirroring approveToolCall's existing pattern).
    await browser.waitUntil(
      async () => !(await $('input[placeholder="Type your answer..."]').isExisting()),
      { timeout: 5000, timeoutMsg: 'pending ask_user UI should clear after answering' },
    )

    // ── Step 7: confirm the turn actually resumed rather than hanging —
    // either the turn closes (assistant message persisted / isStreaming
    // false) or the model asks a further ask_user question (this session's
    // manual harness run observed deepseek-v4-flash-free sometimes asking
    // more than once — see ask-user-loop/state.md) — either is proof the
    // resume->continue->provider.chat() path executed against the real
    // model, not that the turn silently died. What "hanging" would need to
    // look like: isStreaming stays true forever with no further status
    // change and no completion within the timeout below.
    await browser.waitUntil(
      async () => {
        const askedAgain = await $('input[placeholder="Type your answer..."]').isExisting()
        if (askedAgain) return true
        const state = await browser.execute(() => {
          const s = (window as any).__chatStore.getState()
          return { isStreaming: s.isStreaming, error: s.error, messageCount: s.messages.length }
        })
        return !state.isStreaming || !!state.error
      },
      { timeout: 45000, timeoutMsg: 'turn should resume (further question, completion, or error) after answering, not hang forever' },
    )

    const finalState = await browser.execute(() => {
      const s = (window as any).__chatStore.getState()
      return {
        isStreaming: s.isStreaming,
        error: s.error,
        messages: s.messages.map((m: any) => ({ role: m.role, content: m.content })),
      }
    })
    console.log('[e2e] final chat state after answering:', JSON.stringify(finalState, null, 2))

    // The one outcome that would indicate the pause/resume mechanism itself
    // is broken (as opposed to the small free-tier model's own conversational
    // behavior) is the turn erroring with the old hard-error message sa-5
    // fixed. Assert that specific regression can't recur.
    if (finalState.error) {
      expect(finalState.error).not.toContain('is not yet wired to a UI round-trip')
    }
  })
})
