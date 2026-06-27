import { create } from 'zustand'
import type { SearchMatch } from '@/types/ipc'

export interface SearchGroup {
  file: string
  matches: SearchMatch[]
  expanded: boolean
}

interface SearchStore {
  query: string
  replaceText: string
  path: string
  regex: boolean
  caseSensitive: boolean
  wholeWord: boolean
  glob: string
  contextLines: number
  isSearching: boolean
  results: SearchGroup[]
  setQuery: (query: string) => void
  setReplaceText: (text: string) => void
  setPath: (path: string) => void
  toggleRegex: () => void
  toggleCaseSensitive: () => void
  toggleWholeWord: () => void
  setGlob: (glob: string) => void
  setContextLines: (lines: number) => void
  setResults: (matches: SearchMatch[]) => void
  setIsSearching: (v: boolean) => void
  toggleGroup: (file: string) => void
  clearResults: () => void
}

export const useSearchStore = create<SearchStore>((set) => ({
  query: '',
  replaceText: '',
  path: '',
  regex: false,
  caseSensitive: false,
  wholeWord: false,
  glob: '',
  contextLines: 0,
  isSearching: false,
  results: [],
  setQuery: (query) => set({ query }),
  setReplaceText: (replaceText) => set({ replaceText }),
  setPath: (path) => set({ path }),
  toggleRegex: () => set((s) => ({ regex: !s.regex })),
  toggleCaseSensitive: () => set((s) => ({ caseSensitive: !s.caseSensitive })),
  toggleWholeWord: () => set((s) => ({ wholeWord: !s.wholeWord })),
  setGlob: (glob) => set({ glob }),
  setContextLines: (contextLines) => set({ contextLines }),
  setResults: (matches) =>
    set({
      results: Object.entries(
        matches.reduce<Record<string, SearchMatch[]>>((acc, m) => {
          if (!acc[m.file]) acc[m.file] = []
          acc[m.file].push(m)
          return acc
        }, {}),
      ).map(([file, matches]) => ({ file, matches, expanded: true })),
    }),
  setIsSearching: (isSearching) => set({ isSearching }),
  toggleGroup: (file) =>
    set((s) => ({
      results: s.results.map((g) =>
        g.file === file ? { ...g, expanded: !g.expanded } : g,
      ),
    })),
  clearResults: () => set({ results: [], query: '', replaceText: '' }),
}))
