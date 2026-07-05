import { expect } from '@wdio/globals'

describe('Todo Tree Tool — Crash Reproduction', () => {
  before(async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(async () => {
      const panel = await $('main, #root, .app, [class*="chat"]')
      return panel.isExisting()
    }, { timeout: 15000 })
  })

  it('should load without critical errors', async () => {
    const logs = await browser.getLogs('browser')
    const errors = logs.filter(l => l.level === 'SEVERE')
    expect(errors.length).toBe(0)
  })

  it('should have todo_tree in available tools', async () => {
    const tools: Array<{ name: string }> = await browser.execute(() =>
      (window as any).__TAURI_INTERNALS__.invoke('list_available_tools')
    )
    console.log('Available tools:', JSON.stringify(tools.map(t => t.name)))
    const todoTree = tools.find(t => t.name === 'todo_tree')
    expect(todoTree).toBeTruthy()
  })

  it('should not crash when listing tools repeatedly', async () => {
    for (let i = 0; i < 5; i++) {
      const tools: Array<{ name: string }> = await browser.execute(() =>
        (window as any).__TAURI_INTERNALS__.invoke('list_available_tools')
      )
      expect(tools.some(t => t.name === 'todo_tree')).toBe(true)
    }
  })

  it('should still be responsive after tool checks', async () => {
    const title = await browser.getTitle()
    expect(title).toContain('SquireCLI')
  })
})
