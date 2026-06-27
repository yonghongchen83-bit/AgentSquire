import { create } from 'zustand'

export interface EditorTab {
  id: string
  path: string
  filename: string
  language: string
  isDirty: boolean
  isLoading: boolean
}

interface EditorStore {
  tabs: EditorTab[]
  activeTabId: string | null
  openFile: (path: string) => void
  closeTab: (id: string) => void
  setActiveTab: (id: string) => void
  markDirty: (id: string, dirty: boolean) => void
  setLoading: (id: string, loading: boolean) => void
}

function pathToLanguage(path: string): string {
  const ext = path.split('.').pop()?.toLowerCase() ?? ''
  const map: Record<string, string> = {
    ts: 'typescript', tsx: 'typescript', js: 'javascript', jsx: 'javascript',
    rs: 'rust', py: 'python', go: 'go', java: 'java', rb: 'ruby',
    json: 'json', yaml: 'yaml', yml: 'yaml', md: 'markdown',
    css: 'css', scss: 'scss', html: 'html', xml: 'xml',
    toml: 'toml', sql: 'sql', sh: 'shell', bash: 'shell',
  }
  return map[ext] ?? 'plaintext'
}

export const useEditorStore = create<EditorStore>((set) => ({
  tabs: [],
  activeTabId: null,
  openFile: (path) => set((s) => {
    const existing = s.tabs.find((t) => t.path === path)
    if (existing) return { activeTabId: existing.id }
    const parts = path.replace(/\\/g, '/').split('/')
    const tab: EditorTab = {
      id: path,
      path,
      filename: parts[parts.length - 1] ?? path,
      language: pathToLanguage(path),
      isDirty: false,
      isLoading: true,
    }
    return { tabs: [...s.tabs, tab], activeTabId: tab.id }
  }),
  closeTab: (id) => set((s) => {
    const idx = s.tabs.findIndex((t) => t.id === id)
    const tabs = s.tabs.filter((t) => t.id !== id)
    let activeTabId = s.activeTabId
    if (s.activeTabId === id && tabs.length > 0) {
      const newIdx = Math.min(idx, tabs.length - 1)
      activeTabId = tabs[newIdx].id
    } else if (tabs.length === 0) {
      activeTabId = null
    }
    return { tabs, activeTabId }
  }),
  setActiveTab: (id) => set({ activeTabId: id }),
  markDirty: (id, dirty) => set((s) => ({
    tabs: s.tabs.map((t) => t.id === id ? { ...t, isDirty: dirty } : t),
  })),
  setLoading: (id, loading) => set((s) => ({
    tabs: s.tabs.map((t) => t.id === id ? { ...t, isLoading: loading } : t),
  })),
}))
