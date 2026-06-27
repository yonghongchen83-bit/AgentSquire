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

async function ensureSearchView(): Promise<void> {
  const bodyText = await $('body').getText()
  if (bodyText.includes('SEARCH')) return
  const sidebar = await $('.flex.w-12.flex-col')
  const buttons = await sidebar.$$('button')
  const searchBtn = buttons[1]
  await searchBtn.waitForClickable()
  await searchBtn.click()
  await browser.pause(300)
}

describe('Task-004: Search Panel Searches Project Directory', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('should switch to Search view when Search sidebar icon is clicked', async () => {
    await ensureSearchView()
    const panelText = await $('#left-panel').getText()
    expect(panelText).toContain('SEARCH')
  })

  it('should display Search input and search button', async () => {
    await ensureSearchView()
    const searchInput = await $('#left-panel').$('input[placeholder="Search"]')
    await searchInput.waitForExist({ timeout: 3000 })
    expect(await searchInput.isExisting()).toBe(true)
    expect(await searchInput.getValue()).toBe('')
  })

  it('should allow typing a query and pressing Enter to search', async () => {
    await ensureSearchView()
    const searchInput = await $('#left-panel').$('input[placeholder="Search"]')
    await searchInput.waitForExist({ timeout: 3000 })
    await searchInput.click()
    await searchInput.setValue('testQuery')
    expect(await searchInput.getValue()).toBe('testQuery')
    await browser.keys('Enter')
    await browser.pause(300)
  })

  it('should toggle replace mode', async () => {
    await ensureSearchView()
    const replaceToggle = await $('#left-panel').$('button[title="Toggle replace"]')
    await replaceToggle.waitForClickable()
    const replaceInput = await $('#left-panel').$('input[placeholder="Replace"]')
    expect(await replaceInput.isExisting()).toBe(false)
    await replaceToggle.click()
    await browser.pause(200)
    expect(await replaceInput.isExisting()).toBe(true)
    await replaceToggle.click()
    await browser.pause(200)
    expect(await replaceInput.isExisting()).toBe(false)
  })

  it('should toggle search options', async () => {
    await ensureSearchView()
    const optionsToggle = await $('#left-panel').$('button[title="Toggle options"]')
    await optionsToggle.waitForClickable()
    await optionsToggle.click()
    await browser.pause(200)
    const leftPanelText = await $('#left-panel').getText()
    expect(leftPanelText).toContain('Glob filter')
    expect(leftPanelText).toContain('Context lines')
    await optionsToggle.click()
    await browser.pause(200)
    const leftPanelText2 = await $('#left-panel').getText()
    expect(leftPanelText2).not.toContain('Glob filter')
  })
})
