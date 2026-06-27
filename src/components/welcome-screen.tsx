import { FolderOpen } from 'lucide-react'

export function WelcomeScreen() {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 text-[#6B7B8D]">
      <div className="w-20 h-20 rounded-full bg-[#E8EDF2] flex items-center justify-center">
        <FolderOpen className="h-10 w-10 text-[#4A90D9]" />
      </div>
      <h2 className="text-xl font-semibold text-[#1A2332]">MyAgent</h2>
      <p className="text-sm">Open a project to get started</p>
    </div>
  )
}
