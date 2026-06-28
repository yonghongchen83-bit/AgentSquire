import { useState, useEffect } from 'react'
import { readFile } from '@/lib/ipc'
import { FileCode } from 'lucide-react'

export function HtmlPreview({ path, onShowCode }: { path: string; onShowCode: () => void }) {
  const [html, setHtml] = useState<string>('')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    readFile(path)
      .then((content) => {
        if (!cancelled) setHtml(content)
      })
      .catch((e) => {
        if (!cancelled) setError(String(e))
      })
    return () => { cancelled = true }
  }, [path])

  if (error) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-sm text-red-400 gap-2">
        <span>Failed to load file</span>
        <span className="text-gray-400">{error}</span>
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-3 py-1 border-b border-[#E8EDF2] bg-[#F5F7FA] shrink-0">
        <span className="text-xs text-gray-500">HTML Preview</span>
        <button
          onClick={onShowCode}
          className="flex items-center gap-1 text-xs text-[#4A90D9] hover:underline"
        >
          <FileCode className="h-3 w-3" />
          Show Code
        </button>
      </div>
      <iframe
        className="flex-1 w-full border-none bg-white"
        srcDoc={html}
        title="HTML Preview"
        sandbox="allow-scripts allow-same-origin"
      />
    </div>
  )
}
