export const CHAT_MODEL_PREF_KEY = 'chat:last-model-selection'
export const CHAT_THINKING_PREF_KEY = 'chat:last-thinking-level'
export const CHAT_SQUIRE_MODE_DEFAULT_KEY = 'chat:last-squire-mode-default'

export function loadStoredSelection(): { provider: string; model: string } {
  if (typeof window === 'undefined') {
    return { provider: '', model: '' }
  }
  try {
    const raw = window.localStorage.getItem(CHAT_MODEL_PREF_KEY)
    if (!raw) return { provider: '', model: '' }
    const parsed = JSON.parse(raw) as { provider?: string; model?: string }
    return {
      provider: parsed.provider ?? '',
      model: parsed.model ?? '',
    }
  } catch {
    return { provider: '', model: '' }
  }
}

export function saveStoredSelection(provider: string, model: string) {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(CHAT_MODEL_PREF_KEY, JSON.stringify({ provider, model }))
  } catch {
    // Ignore storage errors (private mode/quota/etc.)
  }
}

export function loadStoredThinkingLevel(): 'none' | 'low' | 'mid' | 'high' {
  if (typeof window === 'undefined') return 'mid'
  try {
    const raw = window.localStorage.getItem(CHAT_THINKING_PREF_KEY)
    if (raw === 'none' || raw === 'low' || raw === 'mid' || raw === 'high') {
      return raw
    }
  } catch {
    // ignore
  }
  return 'mid'
}

export function saveStoredThinkingLevel(level: 'none' | 'low' | 'mid' | 'high') {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(CHAT_THINKING_PREF_KEY, level)
  } catch {
    // ignore
  }
}

// session-ux-polish: persists the last-chosen state of ConversationSidebar's Squire-mode
// creation toggle, so it survives a remount (e.g. app restart) instead of always resetting
// to Legacy/off. The first-run default (no stored value yet) remains `false` (Legacy),
// unchanged from session-creation-ux's original design — this only makes an
// already-expressed preference sticky.
export function loadStoredSquireModeDefault(): boolean {
  if (typeof window === 'undefined') return false
  try {
    return window.localStorage.getItem(CHAT_SQUIRE_MODE_DEFAULT_KEY) === 'true'
  } catch {
    return false
  }
}

export function saveStoredSquireModeDefault(value: boolean) {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(CHAT_SQUIRE_MODE_DEFAULT_KEY, value ? 'true' : 'false')
  } catch {
    // ignore
  }
}
