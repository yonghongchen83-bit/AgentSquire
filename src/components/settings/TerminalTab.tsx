import type { AppConfig } from '@/types/ipc'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

export function TerminalTab({
  config,
  updateTerminalShell,
  updateTerminalFontSize,
}: {
  config: AppConfig
  updateTerminalShell: (value: string) => void
  updateTerminalFontSize: (value: number) => void
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-3">
        <Label>Shell Path</Label>
        <Input
          value={config.terminalShell ?? ''}
          onChange={(e) => updateTerminalShell(e.target.value)}
          placeholder="e.g. /bin/bash, C:\\Windows\\System32\\cmd.exe"
        />
        <p className="text-xs text-muted-foreground">
          Leave empty to use system default shell.
        </p>
      </div>

      <div className="space-y-3">
        <Label>Terminal Font Size</Label>
        <div className="flex items-center gap-2">
          <Input
            type="number"
            min={10}
            max={32}
            value={config.terminalFontSize ?? 13}
            onChange={(e) => updateTerminalFontSize(Number(e.target.value))}
            className="w-20"
          />
          <span className="text-sm text-muted-foreground">px</span>
        </div>
      </div>
    </div>
  )
}
