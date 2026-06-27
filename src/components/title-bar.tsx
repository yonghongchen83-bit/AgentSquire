import { PanelLeft } from 'lucide-react'
import { useLayoutStore } from '@/stores/ui-store'

export function TitleBar() {
  const toggleLeftPanel = useLayoutStore((s) => s.toggleLeftPanel)

  return (
    <div
      data-tauri-drag-region
      className="flex h-8 items-center justify-between bg-[#4A90D9] px-3 select-none"
    >
      <button
        onClick={toggleLeftPanel}
        className="flex items-center justify-center w-6 h-6 rounded hover:bg-white/20 text-white"
      >
        <PanelLeft className="h-4 w-4" />
      </button>
      <span className="text-sm font-medium text-white">MyAgent</span>
      <div className="flex items-center gap-1">
        <div className="inline-flex gap-1">
          <div className="w-3 h-3 rounded-full bg-white/30" />
          <div className="w-3 h-3 rounded-full bg-white/30" />
          <div className="w-3 h-3 rounded-full bg-white/30" />
        </div>
      </div>
    </div>
  )
}
