import { expect } from '@wdio/globals'

async function waitForAppReady(): Promise<void> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => {
      const exists = await $('#left-panel').isExisting()
      return exists
    },
    { timeout: 15000, timeoutMsg: 'App did not render left panel within 15s' },
  )
}

describe('Task-003: File Explorer Populated After Project Opened', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('should render FileTree component in the Explorer view', async () => {
    const leftPanel = await $('#left-panel')
    const fileTreeDiv = await leftPanel.$('.overflow-auto.py-1')
    await fileTreeDiv.waitForExist({ timeout: 3000 })
    expect(await fileTreeDiv.isExisting()).toBe(true)
  })

  it('should show files or error state after project opened', async () => {
    await browser.execute((path) => {
      localStorage.setItem('myagent_recent_projects', JSON.stringify([path]))
    }, 'D:\\work\\MyAgent')
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(
      async () => $('#left-panel').isExisting(),
      { timeout: 10000 },
    )
    const recentBtn = await $('button=D:\\work\\MyAgent')
    await recentBtn.waitForClickable()
    await recentBtn.click()
    await browser.pause(1000)
    const fileTreeDiv = await $('#left-panel').$('.overflow-auto.py-1')
    await fileTreeDiv.waitForExist({ timeout: 3000 })
    expect(await fileTreeDiv.isExisting()).toBe(true)

    const text = await fileTreeDiv.getText()
    const hasFilesOrMessage = text.length > 0 && (
      text.includes('.') || text.includes('src') || text.includes('package') ||
      text.includes('No project') || text.includes('Unable to list') || text.includes('Empty directory')
    )
    expect(hasFilesOrMessage).toBe(true)
  })

  it('should switch between sidebar views', async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(
      async () => $('#left-panel').isExisting(),
      { timeout: 10000 },
    )
    const sidebar = await $('.w-12.flex-col')
    const searchBtn = await sidebar.$$('button')[1]
    await searchBtn.waitForClickable()
    await searchBtn.click()
    await browser.pause(300)
    const bodyText = await $('body').getText()
    expect(bodyText).toContain('SEARCH')

    const explorerBtn = await sidebar.$$('button')[0]
    await explorerBtn.waitForClickable()
    await explorerBtn.click()
    await browser.pause(300)
    const bodyText2 = await $('body').getText()
    expect(bodyText2).toContain('EXPLORER')
  })
})
