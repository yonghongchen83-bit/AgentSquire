import { useState, useEffect } from 'react'
import { FolderOpen, Settings, Wifi, History } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'
import { useLayoutStore } from '@/stores/ui-store'
import { useSettingsStore } from '@/stores/settings-store'

const RECENT_KEY = 'myagent_recent_projects'

function getRecentProjects(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) || '[]')
  } catch {
    return []
  }
}

function addRecentProject(path: string) {
  const projects = getRecentProjects().filter((p) => p !== path)
  projects.unshift(path)
  localStorage.setItem(RECENT_KEY, JSON.stringify(projects.slice(0, 10)))
}

export function WelcomeScreen() {
  const setProjectPath = useLayoutStore((s) => s.setProjectPath)
  const setSettingsOpen = useSettingsStore((s) => s.setOpen)
  const [recentProjects, setRecentProjects] = useState<string[]>([])

  useEffect(() => {
    setRecentProjects(getRecentProjects())
  }, [])

  const handleOpenProject = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Open Project',
      })
      if (selected) {
        addRecentProject(selected)
        setRecentProjects(getRecentProjects())
        setProjectPath(selected)
      }
    } catch {
      // Not running in Tauri or dialog cancelled
    }
  }

  const handleOpenRecent = (path: string) => {
    addRecentProject(path)
    setProjectPath(path)
  }

  return (
    <div className="flex flex-col items-center justify-center h-full gap-5 text-[#6B7B8D]">
      <div className="w-20 h-20 rounded-full bg-[#E8EDF2] flex items-center justify-center">
        <FolderOpen className="h-10 w-10 text-[#4A90D9]" />
      </div>
      <h2 className="text-xl font-semibold text-[#1A2332]">MyAgent</h2>
      <p className="text-sm">Open a project to get started</p>
      <button
        onClick={handleOpenProject}
        className="mt-1 px-4 py-2 text-sm font-medium text-white bg-[#4A90D9] rounded-md hover:bg-[#3A7BC8] transition-colors"
      >
        Open Project
      </button>

      {recentProjects.length > 0 && (
        <div className="w-72 mt-2">
          <div className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider mb-2">
            <History className="h-3 w-3" />
            Recent Projects
          </div>
          <div className="space-y-0.5">
            {recentProjects.map((path) => (
              <button
                key={path}
                onClick={() => handleOpenRecent(path)}
                className="w-full text-left px-2 py-1 text-sm truncate rounded hover:bg-[#E8EDF2] text-[#1A2332] transition-colors"
              >
                {path}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="flex items-center gap-3 mt-2">
        <button
          onClick={() => setSettingsOpen(true, 'llm')}
          className="flex items-center gap-1.5 text-xs text-[#4A90D9] hover:underline"
        >
          <Settings className="h-3.5 w-3.5" />
          Model Configuration
        </button>
        <span className="text-[#D6DEE8]">|</span>
        <button
          onClick={() => setSettingsOpen(true, 'llm')}
          className="flex items-center gap-1.5 text-xs text-[#4A90D9] hover:underline"
        >
          <Wifi className="h-3.5 w-3.5" />
          Test Connection
        </button>
      </div>
    </div>
  )
}
