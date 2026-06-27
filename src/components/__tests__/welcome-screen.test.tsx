import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { WelcomeScreen } from '@/components/welcome-screen'

describe('WelcomeScreen', () => {
  it('renders heading and message', () => {
    render(<WelcomeScreen />)
    expect(screen.getByText('MyAgent')).toBeInTheDocument()
    expect(screen.getByText('Open a project to get started')).toBeInTheDocument()
  })
})
