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

describe('Task-005: Bottom Panel Visibility and Toggle', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('should be hidden by default', async () => {
    const bottomPanel = await $('#bottom-panel')
    expect(await bottomPanel.isExisting()).toBe(false)
    const bodyText = await $('body').getText()
    expect(bodyText).toContain('Show Terminal')
    expect(bodyText).not.toContain('Hide Terminal')
  })

  it('should show, display tabs, and hide via StatusBar', async () => {
    const showBtn = await $('button=Show Terminal')
    await showBtn.waitForClickable()
    await showBtn.click()
    await browser.pause(1000)
    await $('#bottom-panel').waitForDisplayed({ timeout: 5000 })
    expect(await $('#bottom-panel').isDisplayed()).toBe(true)

    const bodyText = await $('body').getText()
    expect(bodyText).toContain('Terminal')
    expect(bodyText).toContain('Output')
    expect(bodyText).toContain('Errors')

    const hideBtn = await $('button=Hide Terminal')
    await hideBtn.waitForClickable()
    await hideBtn.click()
    await browser.pause(500)
    expect(await $('#bottom-panel').isExisting()).toBe(false)
    expect(await $('body').getText()).toContain('Show Terminal')
  })
})
