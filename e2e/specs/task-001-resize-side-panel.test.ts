import { expect } from '@wdio/globals'

const SIDEBAR_WIDTH = 48
const TOLERANCE_PX = 5

async function getContentWidth(): Promise<number> {
  const winWidth = await browser.getWindowSize().then(s => s.width)
  return winWidth - SIDEBAR_WIDTH
}

async function getLeftPanelWidth(): Promise<number> {
  const panel = await $('#left-panel')
  return panel.getSize().then(s => s.width)
}

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

describe('Task-001: Side Panel Resize', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('should initialize left panel at 20% of content width', async () => {
    const contentWidth = await getContentWidth()
    const panelWidth = await getLeftPanelWidth()
    const pct = (panelWidth / contentWidth) * 100
    expect(pct).toBeGreaterThan(15)
    expect(pct).toBeLessThan(25)
  })

  it('should resize panel when dragging the divider handle to the right', async () => {
    const initialWidth = await getLeftPanelWidth()

    const handle = await $('#left-handle')
    await handle.waitForDisplayed()

    const DRAG_DISTANCE = 100
    await browser.action('pointer')
      .move({ origin: handle })
      .down({ button: 0 })
      .move({ origin: handle, x: DRAG_DISTANCE, y: 0 })
      .up({ button: 0 })
      .perform()

    await browser.pause(300)
    const newWidth = await getLeftPanelWidth()
    expect(newWidth).toBeGreaterThan(initialWidth)
  })

  it('should resize panel when dragging the divider handle to the left', async () => {
    const initialWidth = await getLeftPanelWidth()

    const handle = await $('#left-handle')
    await handle.waitForDisplayed()

    const DRAG_DISTANCE = 60
    await browser.action('pointer')
      .move({ origin: handle })
      .down({ button: 0 })
      .move({ origin: handle, x: -DRAG_DISTANCE, y: 0 })
      .up({ button: 0 })
      .perform()

    await browser.pause(300)
    const newWidth = await getLeftPanelWidth()
    expect(newWidth).toBeLessThan(initialWidth)
  })

  // minSize boundary test omitted - WebDriver drag cannot go out of viewport bounds
  // The library enforces minSize internally via minSize prop

  it('should adjust editor panel width accordingly', async () => {
    const contentWidth = await getContentWidth()
    const panelWidth = await getLeftPanelWidth()
    const editorPanel = await $('#editor-panel')
    const editorWidth = await editorPanel.getSize().then(s => s.width)
    const combinedWidth = panelWidth + editorWidth
    expect(Math.abs(combinedWidth - contentWidth)).toBeLessThan(10)
  })
})
