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

async function setupProject(): Promise<WebdriverIO.Element> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => $('#left-panel').isExisting(),
    { timeout: 10000 },
  )
  await browser.execute((path) => {
    const store = (window as any).__layoutStore
    if (store) store.getState().setProjectPath(path)
  }, 'D:\\work\\MyAgent')
  await browser.pause(2000)
  const tree = await $('div.h-full.overflow-auto.py-1')
  await tree.waitForExist({ timeout: 5000 })
  return tree
}

describe('Task-006: File Explorer Icons and Directory Expansion', () => {
  it('should show folder icons in blue and file extension icons', async () => {
    const tree = await setupProject()

    const folderIcons = await tree.$$('svg.lucide-folder, svg.lucide-folder-open')
    for (const icon of folderIcons) {
      const color = await icon.getCSSProperty('color')
      const normalized = color.value.replace(/\s/g, '')
      expect(normalized === 'rgb(74,144,217)' || normalized === 'rgba(74,144,217,1)').toBe(true)
    }

    const fileIcons = await tree.$$('svg.lucide-file-code, svg.lucide-file-json, svg.lucide-file-text, svg.lucide-file-image, svg.lucide-file')
    expect(fileIcons.length).toBeGreaterThan(0)
  })

  it('should expand a directory on click showing nested children', async () => {
    const tree = await setupProject()

    const folderIcon = await tree.$('svg.lucide-folder')
    if (!(await folderIcon.isExisting())) {
      console.log('No collapsed folder found — skipping')
      return
    }

    await folderIcon.click()
    await browser.pause(1000)

    const downChevrons = await tree.$$('svg.lucide-chevron-down')
    expect(downChevrons.length).toBeGreaterThan(0)

    const openIcons = await tree.$$('svg.lucide-folder-open')
    expect(openIcons.length).toBeGreaterThan(0)
  })

  it('should collapse a directory on second click', async () => {
    const tree = await setupProject()

    const folderIcon = await tree.$('svg.lucide-folder')
    if (!(await folderIcon.isExisting())) {
      console.log('No folders found — skipping')
      return
    }

    await folderIcon.click()
    await browser.pause(1000)

    const openIcon = await tree.$('svg.lucide-folder-open')
    expect(await openIcon.isExisting()).toBe(true)

    await openIcon.click()
    await browser.pause(800)
    expect(await tree.$('svg.lucide-folder-open').isExisting()).toBe(false)
  })

  it('should show symlink indicator for symlinked files or folders', async () => {
    await setupProject()

    const allSpans = await $$('span')
    const symlinkEls: any[] = []
    for (const span of allSpans) {
      const text = await span.getText()
      if (text.trim() === 'symlink') symlinkEls.push(span)
    }

    if (symlinkEls.length > 0) {
      const linkOverlays = await $$('svg.lucide-link')
      expect(linkOverlays.length).toBeGreaterThan(0)
    } else {
      console.log('No symlinks in project — skipping')
    }
  })
})
