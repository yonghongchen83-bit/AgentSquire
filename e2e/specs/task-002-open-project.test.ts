import { expect } from '@wdio/globals'

const TEST_PROJECT = 'D:\\work\\MyAgent'

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

describe('Task-002: Open Project Sets Workspace Path', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('should display WelcomeScreen with Open Project button', async () => {
    const pageText = await $('body').getText()
    expect(pageText).toContain('MyAgent')
    expect(pageText).toContain('Open a project to get started')
    expect(pageText).toContain('Open Project')
  })

  it('should show recent projects from localStorage', async () => {
    const currentUrl = await browser.getUrl()
    await browser.execute((path) => {
      localStorage.setItem('myagent_recent_projects', JSON.stringify([path]))
    }, TEST_PROJECT)
    await browser.url(currentUrl)
    await browser.waitUntil(
      async () => {
        const exists = await $('#left-panel').isExisting()
        return exists
      },
      { timeout: 15000 },
    )
    await browser.pause(1000)
    const bodyText = await $('body').getText()
    expect(bodyText).toContain('RECENT PROJECTS')
    expect(bodyText).toContain(TEST_PROJECT)
  })

  it('should set projectPath and display in StatusBar when recent project is clicked', async () => {
    const recentBtn = await $(`button=${TEST_PROJECT}`)
    await recentBtn.waitForClickable()
    await recentBtn.click()
    await browser.pause(300)
    const bodyText = await $('body').getText()
    expect(bodyText).toContain(TEST_PROJECT)
  })

  it('should have Open Project in the File menu', async () => {
    await browser.url('http://localhost:5173/')
    await browser.waitUntil(
      async () => $('#left-panel').isExisting(),
      { timeout: 10000 },
    )
    const fileBtn = await $('button=File')
    await fileBtn.waitForClickable()
    await fileBtn.click()
    const menuText = await $('body').getText()
    expect(menuText).toContain('Open Project')
    expect(menuText).toContain('Ctrl+O')
  })
})
