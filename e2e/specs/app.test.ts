import { expect } from '@wdio/globals'

describe('SquireCLI App', () => {
  it('should display the app title', async () => {
    const title = await browser.getTitle()
    expect(title).toContain('SquireCLI')
  })

  it('should have a visible main container', async () => {
    const main = await $('main, #root, .app')
    await expect(main).toBeDisplayed()
  })

  it('should load without critical errors', async () => {
    const logs = await browser.getLogs('browser')
    const errors = logs.filter(l => l.level === 'SEVERE')
    expect(errors.length).toBe(0)
  })
})

describe('UI Components', () => {
  it('should have a working chat panel', async () => {
    const chatPanel = await $('[data-testid="chat-panel"], .chat-panel, main')
    await expect(chatPanel).toBeDisplayed()
  })

  it('should have a chat input', async () => {
    const input = await $('[data-testid="chat-input"], textarea, [contenteditable="true"]')
    await expect(input).toBeDisplayed()
    await expect(input).toBeEnabled()
  })
})
