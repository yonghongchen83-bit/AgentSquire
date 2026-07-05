import { useEffect, useState } from 'react'
import { loadConfig, saveConfig } from '@/lib/ipc'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

type PrefetchDraft = {
  memoryTopK: number
  workflowTopK: number
  toolTopK: number
  skillTopK: number
}

const DEFAULTS: PrefetchDraft = {
  memoryTopK: 10,
  workflowTopK: 3,
  toolTopK: 3,
  skillTopK: 3,
}

function clampPositiveInt(value: number, fallback: number): number {
  if (!Number.isFinite(value)) return fallback
  const rounded = Math.floor(value)
  if (rounded < 1) return fallback
  if (rounded > 100) return 100
  return rounded
}

export function SquireSettingsPanel() {
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState<string | null>(null)
  const [draft, setDraft] = useState<PrefetchDraft>(DEFAULTS)

  useEffect(() => {
    const load = async () => {
      setLoading(true)
      setError(null)
      try {
        const cfg = await loadConfig()
        setDraft({
          memoryTopK: cfg.squirePrefetch?.memoryTopK ?? DEFAULTS.memoryTopK,
          workflowTopK: cfg.squirePrefetch?.workflowTopK ?? DEFAULTS.workflowTopK,
          toolTopK: cfg.squirePrefetch?.toolTopK ?? DEFAULTS.toolTopK,
          skillTopK: cfg.squirePrefetch?.skillTopK ?? DEFAULTS.skillTopK,
        })
      } catch (e) {
        setError(String(e))
      } finally {
        setLoading(false)
      }
    }
    void load()
  }, [])

  const onSave = async () => {
    setSaving(true)
    setError(null)
    setSaved(null)
    try {
      const normalized = {
        memoryTopK: clampPositiveInt(draft.memoryTopK, DEFAULTS.memoryTopK),
        workflowTopK: clampPositiveInt(draft.workflowTopK, DEFAULTS.workflowTopK),
        toolTopK: clampPositiveInt(draft.toolTopK, DEFAULTS.toolTopK),
        skillTopK: clampPositiveInt(draft.skillTopK, DEFAULTS.skillTopK),
      }
      await saveConfig({ squirePrefetch: normalized })
      setDraft(normalized)
      setSaved('Saved')
      window.setTimeout(() => setSaved(null), 1400)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[#6B7B8D]">
        Loading Squire settings...
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-border px-3 py-2">
        <h3 className="text-sm font-semibold">Squire Settings</h3>
        <p className="text-xs text-[#6B7B8D]">Global semantic prefetch defaults for Squire turns</p>
      </div>

      {error && (
        <div className="border-b border-border bg-red-50 px-3 py-2 text-xs text-red-600">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-3 space-y-4">
        <div className="rounded-lg border border-border p-3 space-y-3">
          <div className="space-y-1">
            <Label htmlFor="memoryTopK">Memory Prefetch (Top K)</Label>
            <Input
              id="memoryTopK"
              type="number"
              min={1}
              max={100}
              value={draft.memoryTopK}
              onChange={(e) => setDraft((d) => ({ ...d, memoryTopK: Number(e.target.value) }))}
            />
          </div>

          <div className="space-y-1">
            <Label htmlFor="workflowTopK">Workflow Prefetch (Top K)</Label>
            <Input
              id="workflowTopK"
              type="number"
              min={1}
              max={100}
              value={draft.workflowTopK}
              onChange={(e) => setDraft((d) => ({ ...d, workflowTopK: Number(e.target.value) }))}
            />
          </div>

          <div className="space-y-1">
            <Label htmlFor="toolTopK">Tool Prefetch (Top K)</Label>
            <Input
              id="toolTopK"
              type="number"
              min={1}
              max={100}
              value={draft.toolTopK}
              onChange={(e) => setDraft((d) => ({ ...d, toolTopK: Number(e.target.value) }))}
            />
          </div>

          <div className="space-y-1">
            <Label htmlFor="skillTopK">Skill Prefetch (Top K)</Label>
            <Input
              id="skillTopK"
              type="number"
              min={1}
              max={100}
              value={draft.skillTopK}
              onChange={(e) => setDraft((d) => ({ ...d, skillTopK: Number(e.target.value) }))}
            />
          </div>

          <div className="text-[11px] text-[#6B7B8D]">
            Preserved tokens are always merged first, then prefetch results are appended with dedupe.
          </div>
        </div>
      </div>

      <div className="border-t border-border px-3 py-2 flex items-center justify-end gap-2">
        {saved && <span className="text-xs text-green-700">{saved}</span>}
        <Button size="sm" onClick={onSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save'}
        </Button>
      </div>
    </div>
  )
}
