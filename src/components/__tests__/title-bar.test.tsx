import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TitleBar } from '@/components/title-bar'
import { useLayoutStore } from '@/stores/ui-store'

describe('TitleBar', () => {
  it('renders app name', () => {
    render(<TitleBar />)
    expect(screen.getByText('MyAgent')).toBeInTheDocument()
  })

  it('toggles left panel when hamburger clicked', async () => {
    const user = userEvent.setup()
    render(<TitleBar />)
    expect(useLayoutStore.getState().leftPanelVisible).toBe(true)
    await user.click(screen.getByRole('button'))
    expect(useLayoutStore.getState().leftPanelVisible).toBe(false)
  })
})
