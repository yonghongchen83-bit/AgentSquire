import { FolderOpen } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'
import { useLayoutStore } from '@/stores/ui-store'

export function WelcomeScreen() {
  const setProjectPath = useLayoutStore((s) => s.setProjectPath)

  const handleOpenProject = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Open Project',
      })
      if (selected) {
        setProjectPath(selected)
      }
    } catch {
      // Not running in Tauri or dialog cancelled
    }
  }

  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 text-[#6B7B8D]">
      <div className="w-20 h-20 rounded-full bg-[#E8EDF2] flex items-center justify-center">
        <FolderOpen className="h-10 w-10 text-[#4A90D9]" />
      </div>
      <h2 className="text-xl font-semibold text-[#1A2332]">MyAgent</h2>
      <p className="text-sm">Open a project to get started</p>
      <button
        onClick={handleOpenProject}
        className="mt-2 px-4 py-2 text-sm font-medium text-white bg-[#4A90D9] rounded-md hover:bg-[#3A7BC8] transition-colors"
      >
        Open Project
      </button>
    </div>
  )
}
