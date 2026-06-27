import { describe, it, expect, beforeEach } from 'vitest'
import { render } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { KeyboardShortcuts } from '@/components/keyboard-shortcuts'
import { useLayoutStore } from '@/stores/ui-store'
import { useSettingsStore } from '@/stores/settings-store'

describe('KeyboardShortcuts', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      leftPanelVisible: true,
      bottomPanelVisible: false,
      leftPanelActiveView: 'explorer',
    })
    useSettingsStore.setState({ open: false })
  })

  it('Ctrl+Shift+P opens settings', async () => {
    render(<KeyboardShortcuts />)
    await userEvent.keyboard('{Control>}{Shift>}P{/Control}{/Shift}')
    expect(useSettingsStore.getState().open).toBe(true)
  })

  it('Ctrl+` toggles bottom panel', async () => {
    render(<KeyboardShortcuts />)
    expect(useLayoutStore.getState().bottomPanelVisible).toBe(false)
    await userEvent.keyboard('{Control>}`{/Control}')
    expect(useLayoutStore.getState().bottomPanelVisible).toBe(true)
  })

  it('Ctrl+Shift+F switches to search view', async () => {
    render(<KeyboardShortcuts />)
    await userEvent.keyboard('{Control>}{Shift>}F{/Control}{/Shift}')
    expect(useLayoutStore.getState().leftPanelActiveView).toBe('search')
  })

  it('Ctrl+B toggles left panel', async () => {
    render(<KeyboardShortcuts />)
    expect(useLayoutStore.getState().leftPanelVisible).toBe(true)
    await userEvent.keyboard('{Control>}b{/Control}')
    expect(useLayoutStore.getState().leftPanelVisible).toBe(false)
  })
})
