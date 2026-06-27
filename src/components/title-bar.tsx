import { Menu } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'

export function TitleBar() {
  const appWindow = getCurrentWindow()

  return (
    <div
      data-tauri-drag-region
      className="flex h-8 items-center justify-between bg-[#4A90D9] px-3 select-none"
    >
      <button
        className="flex items-center justify-center w-6 h-6 rounded hover:bg-white/20 text-white"
      >
        <Menu className="h-4 w-4" />
      </button>
      <span className="text-sm font-medium text-white">MyAgent</span>
      <div className="flex items-center gap-1" data-tauri-drag-region>
        <button
          onClick={() => appWindow.minimize()}
          className="flex items-center justify-center w-5 h-5 rounded hover:bg-white/20 text-white/70 hover:text-white transition-colors"
        >
          <svg viewBox="0 0 12 12" className="w-3 h-3" fill="currentColor"><rect y="5" width="12" height="1.5" rx="0.5" /></svg>
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="flex items-center justify-center w-5 h-5 rounded hover:bg-white/20 text-white/70 hover:text-white transition-colors"
        >
          <svg viewBox="0 0 12 12" className="w-3 h-3" fill="currentColor"><rect x="1" y="1" width="10" height="10" rx="1" stroke="currentColor" strokeWidth="1.5" fill="none" /></svg>
        </button>
        <button
          onClick={() => appWindow.close()}
          className="flex items-center justify-center w-5 h-5 rounded hover:bg-red-500 text-white/70 hover:text-white transition-colors"
        >
          <svg viewBox="0 0 12 12" className="w-3 h-3" fill="currentColor"><path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.5" fill="none" /></svg>
        </button>
      </div>
    </div>
  )
}
