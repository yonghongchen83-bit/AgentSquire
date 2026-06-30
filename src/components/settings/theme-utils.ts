import { invoke } from '@tauri-apps/api/core'
import type { AppConfig } from '@/types/ipc'

export function applyThemeClass(theme: string) {
  const root = document.documentElement
  if (theme === 'dark') {
    root.classList.add('dark')
  } else if (theme === 'light') {
    root.classList.remove('dark')
  } else {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    root.classList.toggle('dark', prefersDark)
  }
}

export function initTheme() {
  invoke<AppConfig>('get_config').then((config) => {
    applyThemeClass(config.theme)
  }).catch(() => {})
}
