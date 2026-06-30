import { expect } from '@wdio/globals'

const SIDEBAR_WIDTH = 48
const TOLERANCE = 2

async function waitForAppReady(): Promise<void> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => {
      const hasStore = await browser.execute(() => Boolean((window as any).__layoutStore))
      const hasLeft = await $('#left-panel').isExisting()
      return hasStore && hasLeft
    },
    { timeout: 15000, timeoutMsg: 'App did not become ready within 15s' },
  )
}

async function ensurePanelsVisible(): Promise<void> {
  await browser.execute(() => {
    const store = (window as any).__layoutStore
    if (!store) return
    store.setState({
      leftPanelVisible: true,
      leftPanelActiveView: 'explorer',
      rightPanelVisible: true,
      bottomPanelVisible: true,
    })
  })

  await $('#left-panel').waitForDisplayed({ timeout: 5000 })
  await $('#right-panel').waitForDisplayed({ timeout: 5000 })
  await $('#bottom-panel').waitForDisplayed({ timeout: 5000 })
}

async function dragHandle(handleId: string, dx: number, dy: number): Promise<void> {
  const handle = await $(`#${handleId}`)
  await handle.waitForDisplayed({ timeout: 5000 })

  await browser.action('pointer')
    .move({ origin: handle })
    .down({ button: 0 })
    .move({ origin: handle, x: dx, y: dy })
    .up({ button: 0 })
    .perform()

  await browser.pause(200)
}

async function getSavedPanelLayout(): Promise<{ left: number; right: number; bottom: number }> {
  return browser.execute(() => {
    const s = (window as any).__layoutStore?.getState()
    return {
      left: s?.leftPanelWidth ?? 0,
      right: s?.rightPanelWidth ?? 0,
      bottom: s?.bottomPanelHeight ?? 0,
    }
  })
}

async function getContentWidth(): Promise<number> {
  const winWidth = await browser.getWindowSize().then((s) => s.width)
  return winWidth - SIDEBAR_WIDTH
}

describe('Task-008: Panel Size Persistence', () => {
  before(async () => {
    await waitForAppReady()
    await ensurePanelsVisible()
  })

  it('should remember left, right, and bottom panel sizes after reload', async () => {
    // Resize left, right, and bottom panels to non-default sizes.
    await dragHandle('left-handle', 120, 0)
    await dragHandle('right-handle', -120, 0)
    await dragHandle('editor-handle', 0, -120)

    // Debounced save in App.tsx waits 500ms.
    await browser.pause(900)

    const saved = await getSavedPanelLayout()
    expect(saved.left).toBeGreaterThan(0)
    expect(saved.right).toBeGreaterThan(0)
    expect(saved.bottom).toBeGreaterThan(0)

    // Reload and verify store values were restored from persisted config.
    await waitForAppReady()
    const restored = await getSavedPanelLayout()

    expect(Math.abs(restored.left - saved.left)).toBeLessThanOrEqual(TOLERANCE)
    expect(Math.abs(restored.right - saved.right)).toBeLessThanOrEqual(TOLERANCE)
    expect(Math.abs(restored.bottom - saved.bottom)).toBeLessThanOrEqual(TOLERANCE)

    // Also verify the rendered layout reflects restored values.
    await ensurePanelsVisible()

    const contentWidth = await getContentWidth()
    const leftPx = await $('#left-panel').getSize().then((s) => s.width)
    const rightPx = await $('#right-panel').getSize().then((s) => s.width)
    const topPx = await $('#top-area').getSize().then((s) => s.height)
    const bottomPx = await $('#bottom-panel').getSize().then((s) => s.height)

    const leftPct = (leftPx / contentWidth) * 100
    const rightPct = (rightPx / contentWidth) * 100
    const bottomPct = (bottomPx / (topPx + bottomPx)) * 100

    expect(Math.abs(leftPct - restored.left)).toBeLessThanOrEqual(3)
    expect(Math.abs(rightPct - restored.right)).toBeLessThanOrEqual(3)
    expect(Math.abs(bottomPct - restored.bottom)).toBeLessThanOrEqual(3)
  })
})
