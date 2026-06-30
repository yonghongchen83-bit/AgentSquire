export const CHAT_MODEL_PREF_KEY = 'chat:last-model-selection'
export const CHAT_THINKING_PREF_KEY = 'chat:last-thinking-level'

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
