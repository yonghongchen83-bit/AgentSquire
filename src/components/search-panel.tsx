import { useState, useCallback } from 'react'
import { Search, X, ChevronRight, ChevronDown, FileText, Replace, ReplaceAll, Settings2 } from 'lucide-react'
import { useSearchStore } from '@/stores/search-store'
import { useEditorStore } from '@/stores/editor-store'
import { searchFiles, replaceInFiles } from '@/lib/ipc'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'

function ToggleChip({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`px-2 py-0.5 text-xs rounded font-mono transition-colors ${
        active
          ? 'bg-[#4A90D9] text-white'
          : 'bg-[#E8EDF2] text-[#6B7B8D] hover:bg-[#D0DCE8]'
      }`}
    >
      {label}
    </button>
  )
}

export function SearchPanel() {
  const query = useSearchStore((s) => s.query)
  const replaceText = useSearchStore((s) => s.replaceText)
  const path = useSearchStore((s) => s.path)
  const regex = useSearchStore((s) => s.regex)
  const caseSensitive = useSearchStore((s) => s.caseSensitive)
  const wholeWord = useSearchStore((s) => s.wholeWord)
  const glob = useSearchStore((s) => s.glob)
  const contextLines = useSearchStore((s) => s.contextLines)
  const isSearching = useSearchStore((s) => s.isSearching)
  const results = useSearchStore((s) => s.results)
  const setQuery = useSearchStore((s) => s.setQuery)
  const setReplaceText = useSearchStore((s) => s.setReplaceText)
  const toggleRegex = useSearchStore((s) => s.toggleRegex)
  const toggleCaseSensitive = useSearchStore((s) => s.toggleCaseSensitive)
  const toggleWholeWord = useSearchStore((s) => s.toggleWholeWord)
  const setGlob = useSearchStore((s) => s.setGlob)
  const setContextLines = useSearchStore((s) => s.setContextLines)
  const setResults = useSearchStore((s) => s.setResults)
  const setIsSearching = useSearchStore((s) => s.setIsSearching)
  const toggleGroup = useSearchStore((s) => s.toggleGroup)
  const clearResults = useSearchStore((s) => s.clearResults)
  const openFile = useEditorStore((s) => s.openFile)
  const setGotoLine = useEditorStore((s) => s.setGotoLine)

  const [showReplace, setShowReplace] = useState(false)
  const [showOptions, setShowOptions] = useState(false)

  const handleSearch = useCallback(async () => {
    if (!query.trim()) return
    setIsSearching(true)
    try {
      const matches = await searchFiles(query, path || '.', {
        regex,
        caseSensitive,
        wholeWord,
        glob: glob || undefined,
        contextLines: contextLines || undefined,
      })
      setResults(matches)
    } finally {
      setIsSearching(false)
    }
  }, [query, path, regex, caseSensitive, wholeWord, glob, contextLines, setIsSearching, setResults])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleSearch()
  }

  const handleResultClick = (file: string, line: number) => {
    openFile(file)
    setGotoLine(line)
  }

  const handleReplaceAll = async () => {
    if (!query.trim() || !replaceText.trim()) return
    try {
      const count = await replaceInFiles({
        query,
        replacement: replaceText,
        path: path || '.',
        regex,
        case_sensitive: caseSensitive,
        glob: glob || undefined,
      })
      if (count > 0) {
        const matches = await searchFiles(query, path || '.', {
          regex,
          caseSensitive,
          wholeWord,
          glob: glob || undefined,
          contextLines: contextLines || undefined,
        })
        setResults(matches)
      }
    } catch { }
  }

  const totalMatches = results.reduce((acc, g) => acc + g.matches.length, 0)

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-3 h-8 text-xs font-semibold text-[#6B7B8D] uppercase tracking-wider border-b border-border">
        <span>Search</span>
        {results.length > 0 && (
          <button onClick={clearResults} className="hover:text-[#1A2332] transition-colors">
            <X className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      <div className="p-3 space-y-2">
        <div className="relative">
          <Input
            placeholder="Search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            className="pr-8 text-sm h-8"
          />
          <button
            onClick={handleSearch}
            disabled={isSearching}
            className="absolute right-1.5 top-1/2 -translate-y-1/2 text-[#6B7B8D] hover:text-[#1A2332] disabled:opacity-50"
          >
            {isSearching ? (
              <div className="h-3.5 w-3.5 border-2 border-[#4A90D9] border-t-transparent rounded-full animate-spin" />
            ) : (
              <Search className="h-3.5 w-3.5" />
            )}
          </button>
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={() => setShowReplace(!showReplace)}
            className={`p-1 rounded transition-colors ${showReplace ? 'bg-[#D0DCE8] text-[#1A2332]' : 'text-[#6B7B8D] hover:text-[#1A2332]'}`}
            title="Toggle replace"
          >
            <Replace className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={() => setShowOptions(!showOptions)}
            className={`p-1 rounded transition-colors ${showOptions ? 'bg-[#D0DCE8] text-[#1A2332]' : 'text-[#6B7B8D] hover:text-[#1A2332]'}`}
            title="Toggle options"
          >
            <Settings2 className="h-3.5 w-3.5" />
          </button>
          <div className="flex-1" />
          {results.length > 0 && (
            <span className="text-xs text-[#6B7B8D]">
              {totalMatches} {totalMatches === 1 ? 'result' : 'results'}
            </span>
          )}
        </div>

        {showReplace && (
          <div className="flex gap-1">
            <Input
              placeholder="Replace"
              value={replaceText}
              onChange={(e) => setReplaceText(e.target.value)}
              onKeyDown={handleKeyDown}
              className="text-sm h-8 flex-1"
            />
            <Button
              variant="outline"
              size="sm"
              onClick={handleReplaceAll}
              disabled={!query.trim() || !replaceText.trim()}
              className="h-8 px-2"
              title="Replace All"
            >
              <ReplaceAll className="h-3.5 w-3.5" />
            </Button>
          </div>
        )}

        {showOptions && (
          <div className="space-y-2 p-2 bg-[#E8EDF2] rounded text-xs">
            <div className="flex flex-wrap gap-1.5">
              <ToggleChip label="Ab" active={caseSensitive} onClick={toggleCaseSensitive} />
              <ToggleChip label=".*" active={regex} onClick={toggleRegex} />
              <ToggleChip label="W" active={wholeWord} onClick={toggleWholeWord} />
            </div>
            <div className="space-y-1">
              <label className="text-[#6B7B8D] block">Glob filter</label>
              <Input
                placeholder="e.g. *.ts, src/**"
                value={glob}
                onChange={(e) => setGlob(e.target.value)}
                className="text-sm h-7"
              />
            </div>
            <div className="space-y-1">
              <label className="text-[#6B7B8D] block">Context lines</label>
              <Input
                type="number"
                min={0}
                max={10}
                value={contextLines}
                onChange={(e) => setContextLines(Number(e.target.value))}
                className="text-sm h-7 w-20"
              />
            </div>
          </div>
        )}
      </div>

      <ScrollArea className="flex-1">
        {results.length === 0 && !isSearching && (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-[#6B7B8D] p-4">
            <Search className="h-8 w-8" />
            <p className="text-sm">Enter a query to search</p>
          </div>
        )}

        {results.map((group) => (
          <div key={group.file}>
            <button
              onClick={() => toggleGroup(group.file)}
              className="flex items-center gap-1 w-full px-2 py-1 text-xs font-medium text-[#1A2332] hover:bg-[#E8EDF2] transition-colors"
            >
              {group.expanded ? (
                <ChevronDown className="h-3 w-3 shrink-0 text-[#6B7B8D]" />
              ) : (
                <ChevronRight className="h-3 w-3 shrink-0 text-[#6B7B8D]" />
              )}
              <FileText className="h-3 w-3 shrink-0" />
              <span className="truncate flex-1 text-left">{group.file}</span>
              <span className="text-[#6B7B8D] shrink-0">{group.matches.length}</span>
            </button>

            {group.expanded && group.matches.map((match, idx) => (
              <button
                key={`${match.line_number}-${idx}`}
                onClick={() => handleResultClick(match.file, match.line_number)}
                className="flex flex-col w-full px-2 py-0.5 text-xs hover:bg-[#E8EDF2] transition-colors cursor-pointer text-left"
              >
                <div className="flex items-center gap-1 text-[#4A90D9]">
                  <span className="font-mono shrink-0">
                    L{match.line_number}:{match.column}
                  </span>
                </div>
                <div className="font-mono text-[#1A2332] truncate leading-5">
                  {match.content}
                </div>
                {match.context_before.length > 0 && (
                  <div className="text-[#6B7B8D] font-mono truncate leading-4 border-l-2 border-[#D6DEE8] pl-1 mt-0.5 space-y-0.5">
                    {match.context_before.map((ctx, i) => (
                      <div key={i} className="truncate">{ctx}</div>
                    ))}
                  </div>
                )}
                {match.context_after.length > 0 && (
                  <div className="text-[#6B7B8D] font-mono truncate leading-4 border-l-2 border-[#B0C0D0] pl-1 mt-0.5 space-y-0.5">
                    {match.context_after.map((ctx, i) => (
                      <div key={i} className="truncate">{ctx}</div>
                    ))}
                  </div>
                )}
              </button>
            ))}
          </div>
        ))}
      </ScrollArea>
    </div>
  )
}
