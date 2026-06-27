import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { WelcomeScreen } from '@/components/welcome-screen'

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

describe('WelcomeScreen', () => {
  it('renders heading, message, and open project button', () => {
    render(<WelcomeScreen />)
    expect(screen.getByText('MyAgent')).toBeInTheDocument()
    expect(screen.getByText('Open a project to get started')).toBeInTheDocument()
    expect(screen.getByText('Open Project')).toBeInTheDocument()
  })
})
