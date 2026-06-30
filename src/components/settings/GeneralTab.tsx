import { Sun, Moon, Monitor } from 'lucide-react'
import type { AppConfig } from '@/types/ipc'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'

const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor }

function ThemeCard({
  mode,
  current,
  onChange,
}: {
  mode: 'light' | 'dark' | 'system'
  current: string
  onChange: (v: string) => void
}) {
  const Icon = THEME_ICONS[mode]
  const isActive = current === mode
  return (
    <button
      onClick={() => onChange(mode)}
      className={`flex flex-col items-center gap-2 p-4 rounded-lg border-2 transition-all ${
        isActive
          ? 'border-primary bg-primary/5'
          : 'border-border hover:border-muted-foreground/30'
      }`}
    >
      <Icon className={`h-6 w-6 ${isActive ? 'text-primary' : 'text-muted-foreground'}`} />
      <span className={`text-sm font-medium capitalize ${isActive ? 'text-foreground' : 'text-muted-foreground'}`}>
        {mode}
      </span>
    </button>
  )
}

export function GeneralTab({
  config,
  handleThemeChange,
  updateEditorFontSize,
  updateEditorWordWrap,
  updateEditorTabSize,
}: {
  config: AppConfig
  handleThemeChange: (value: string) => void
  updateEditorFontSize: (value: number) => void
  updateEditorWordWrap: (value: boolean) => void
  updateEditorTabSize: (value: number) => void
}) {
  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <Label>Theme</Label>
        <div className="flex gap-3">
          {(['light', 'dark', 'system'] as const).map((mode) => (
            <ThemeCard
              key={mode}
              mode={mode}
              current={config.theme}
              onChange={handleThemeChange}
            />
          ))}
        </div>
      </div>

      <div className="space-y-3">
        <Label>Editor Font Size</Label>
        <div className="flex items-center gap-2">
          <Input
            type="number"
            min={10}
            max={32}
            value={config.fontSize}
            onChange={(e) => updateEditorFontSize(Number(e.target.value))}
            className="w-20"
          />
          <span className="text-sm text-muted-foreground">px</span>
        </div>
      </div>

      <div className="space-y-3">
        <Label>Word Wrap</Label>
        <div className="flex items-center gap-2">
          <Switch
            checked={config.wordWrap ?? false}
            onCheckedChange={updateEditorWordWrap}
          />
          <span className="text-sm text-muted-foreground">
            {config.wordWrap ? 'Enabled' : 'Disabled'}
          </span>
        </div>
      </div>

      <div className="space-y-3">
        <Label>Tab Size</Label>
        <Select
          value={String(config.tabSize ?? 4)}
          onValueChange={(v) => updateEditorTabSize(Number(v))}
        >
          <SelectTrigger className="w-20">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {[2, 4, 6, 8].map((n) => (
              <SelectItem key={n} value={String(n)}>{n}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </div>
  )
}
