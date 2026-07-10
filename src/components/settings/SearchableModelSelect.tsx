import { useState, useRef, useEffect, useMemo } from 'react'
import { Input } from '@/components/ui/input'

interface SearchableModelSelectProps {
  knownModels: { id: string; label?: string }[]
  fetched: string[]
  onSelect: (modelId: string) => void
  onRefresh?: () => void
  fetching?: boolean
}

export function SearchableModelSelect({
  knownModels,
  fetched,
  onSelect,
  onRefresh,
  fetching,
}: SearchableModelSelectProps) {
  const [query, setQuery] = useState('')
  const [isOpen, setIsOpen] = useState(false)
  const [highlightedIndex, setHighlightedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  const allItems = useMemo(() => {
    const known = knownModels.map((m) => ({ id: m.id, label: m.label ?? m.id, group: 'known' as const }))
    if (fetched.length === 0) return known
    const items: { id: string; label: string; group: 'known' | 'sep' | 'fetched' }[] = [...known]
    if (known.length > 0) {
      items.push({ id: '__sep__', label: '--- fetched ---', group: 'sep' })
    }
    for (const m of fetched) {
      if (!known.some((k) => k.id === m)) {
        items.push({ id: m, label: m, group: 'fetched' })
      }
    }
    return items
  }, [knownModels, fetched])

  const filtered = useMemo(() => {
    if (!query.trim()) return allItems
    const q = query.toLowerCase()
    return allItems.filter(
      (item) => item.group === 'sep' || item.label.toLowerCase().includes(q),
    )
  }, [allItems, query])

  const selectableItems = useMemo(
    () => filtered.filter((item) => item.group !== 'sep'),
    [filtered],
  )

  useEffect(() => { setHighlightedIndex(0) }, [query, fetched])

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  const commitSelection = (modelId: string) => {
    onSelect(modelId)
    setQuery('')
    setIsOpen(false)
    inputRef.current?.blur()
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isOpen) {
      if (e.key === 'ArrowDown' || e.key === 'Enter') {
        e.preventDefault()
        setIsOpen(true)
      }
      return
    }
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setHighlightedIndex((p) => Math.min(p + 1, Math.max(selectableItems.length - 1, 0)))
        break
      case 'ArrowUp':
        e.preventDefault()
        setHighlightedIndex((p) => Math.max(p - 1, 0))
        break
      case 'Enter':
        e.preventDefault()
        if (selectableItems.length > 0 && highlightedIndex < selectableItems.length) {
          commitSelection(selectableItems[highlightedIndex].id)
        } else if (query.trim()) {
          commitSelection(query.trim())
        }
        break
      case 'Escape':
        setIsOpen(false)
        break
    }
  }

  return (
    <div ref={containerRef} className="relative flex-1">
      <div className="flex gap-2">
        <Input
          ref={inputRef}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value)
            if (!isOpen) setIsOpen(true)
          }}
          onFocus={() => setIsOpen(true)}
          onKeyDown={handleKeyDown}
          placeholder="Search or type model name..."
          className="flex-1"
        />
        {onRefresh && (
          <button
            onClick={onRefresh}
            disabled={fetching}
            className="inline-flex items-center justify-center rounded-md border border-input bg-background px-2.5 py-2 text-sm font-medium hover:bg-accent hover:text-accent-foreground disabled:opacity-50 shrink-0"
            title="Refresh models from server"
          >
            <svg
              className={`h-4 w-4 ${fetching ? 'animate-spin' : ''}`}
              xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" strokeWidth="2"
              strokeLinecap="round" strokeLinejoin="round"
            >
              <path d="M21 12a9 9 0 1 1-9-9" />
              <path d="M21 3v6h-6" />
            </svg>
          </button>
        )}
      </div>

      {isOpen && fetching && (
        <div className="absolute z-50 mt-1 w-full rounded-md border bg-popover text-popover-foreground shadow-md p-3 text-sm text-muted-foreground text-center">
          Loading models...
        </div>
      )}

      {isOpen && !fetching && (
        <div className="absolute z-50 mt-1 w-full rounded-md border bg-popover text-popover-foreground shadow-md overflow-hidden flex flex-col"
             style={{ maxHeight: 300 }}>
          {selectableItems.length === 0 && query.trim() && (
            <div
              className="flex cursor-pointer select-none items-center rounded-sm px-2 py-1.5 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground shrink-0"
              onMouseDown={(e) => {
                e.preventDefault()
                commitSelection(query.trim())
              }}
            >
              Add &ldquo;{query.trim()}&rdquo;
            </div>
          )}
          <div className="overflow-y-auto p-1">
            {filtered.map((item) => {
              if (item.group === 'sep') {
                return (
                  <div key={item.id} className="px-2 py-1 text-xs text-muted-foreground">
                    — fetched —
                  </div>
                )
              }
              const selIdx = selectableItems.indexOf(item)
              return (
                <div
                  key={item.id}
                  className={`relative flex cursor-default select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none ${
                    selIdx === highlightedIndex
                      ? 'bg-accent text-accent-foreground'
                      : 'hover:bg-accent hover:text-accent-foreground'
                  }`}
                  onMouseEnter={() => setHighlightedIndex(selIdx)}
                  onMouseDown={(e) => {
                    e.preventDefault()
                    commitSelection(item.id)
                  }}
                >
                  {item.label}
                </div>
              )
            })}
            {filtered.length === 0 && !query.trim() && (
              <div className="px-2 py-4 text-sm text-muted-foreground text-center">
                No models available. Click refresh to fetch from server.
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
