import type { AppConfig } from '@/types/ipc'
import { Label } from '@/components/ui/label'

export function SearchTab({
  config,
  updateSearchExclude,
}: {
  config: AppConfig
  updateSearchExclude: (value: string[]) => void
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-3">
        <Label>Exclude Patterns</Label>
        <p className="text-xs text-muted-foreground">
          One pattern per line. These directories are skipped during search.
        </p>
        <textarea
          className="w-full h-24 rounded-md border border-input bg-background px-3 py-2 text-sm resize-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          value={config.searchExclude?.join('\n') ?? ''}
          onChange={(e) => updateSearchExclude(e.target.value.split('\n').filter(Boolean))}
          placeholder="node_modules&#10;.git&#10;target&#10;dist"
        />
      </div>
    </div>
  )
}
