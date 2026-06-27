import { describe, it, expect, beforeEach } from 'vitest'
import { useEditorStore } from './editor-store'

describe('EditorStore', () => {
  beforeEach(() => {
    useEditorStore.setState({ tabs: [], activeTabId: null })
  })

  it('opens a new file tab', () => {
    useEditorStore.getState().openFile('/test/foo.ts')
    const state = useEditorStore.getState()
    expect(state.tabs).toHaveLength(1)
    expect(state.tabs[0].filename).toBe('foo.ts')
    expect(state.tabs[0].language).toBe('typescript')
    expect(state.tabs[0].isLoading).toBe(true)
    expect(state.activeTabId).toBe('/test/foo.ts')
  })

  it('switches to existing tab when opening same file', () => {
    useEditorStore.getState().openFile('/test/foo.ts')
    useEditorStore.getState().openFile('/test/bar.ts')
    useEditorStore.getState().openFile('/test/foo.ts')
    expect(useEditorStore.getState().tabs).toHaveLength(2)
    expect(useEditorStore.getState().activeTabId).toBe('/test/foo.ts')
  })

  it('closes a tab and activates next', () => {
    useEditorStore.getState().openFile('/test/a.ts')
    useEditorStore.getState().openFile('/test/b.ts')
    useEditorStore.getState().openFile('/test/c.ts')
    useEditorStore.getState().closeTab('/test/b.ts')
    expect(useEditorStore.getState().tabs).toHaveLength(2)
    expect(useEditorStore.getState().tabs.map((t) => t.filename)).toEqual(['a.ts', 'c.ts'])
  })

  it('sets active tab', () => {
    useEditorStore.getState().openFile('/test/a.ts')
    useEditorStore.getState().openFile('/test/b.ts')
    useEditorStore.getState().setActiveTab('/test/a.ts')
    expect(useEditorStore.getState().activeTabId).toBe('/test/a.ts')
  })

  it('marks tabs as dirty/clean', () => {
    useEditorStore.getState().openFile('/test/a.ts')
    useEditorStore.getState().markDirty('/test/a.ts', true)
    expect(useEditorStore.getState().tabs[0].isDirty).toBe(true)
    useEditorStore.getState().markDirty('/test/a.ts', false)
    expect(useEditorStore.getState().tabs[0].isDirty).toBe(false)
  })

  it('sets loading state', () => {
    useEditorStore.getState().openFile('/test/a.ts')
    useEditorStore.getState().setLoading('/test/a.ts', false)
    expect(useEditorStore.getState().tabs[0].isLoading).toBe(false)
  })

  it('closes last tab sets activeTabId to null', () => {
    useEditorStore.getState().openFile('/test/a.ts')
    useEditorStore.getState().closeTab('/test/a.ts')
    expect(useEditorStore.getState().tabs).toHaveLength(0)
    expect(useEditorStore.getState().activeTabId).toBeNull()
  })

  it('detects language from extension', () => {
    useEditorStore.getState().openFile('/test/main.rs')
    expect(useEditorStore.getState().tabs[0].language).toBe('rust')
    useEditorStore.getState().openFile('/test/index.html')
    expect(useEditorStore.getState().tabs[1].language).toBe('html')
    useEditorStore.getState().openFile('/test/style.css')
    expect(useEditorStore.getState().tabs[2].language).toBe('css')
    useEditorStore.getState().openFile('/test/unknown.xyz')
    expect(useEditorStore.getState().tabs[3].language).toBe('plaintext')
  })
})
