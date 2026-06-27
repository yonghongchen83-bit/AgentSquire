import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import { SplashScreen } from '@/components/splash-screen'

describe('SplashScreen', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders app name and subtitle', () => {
    render(<SplashScreen onLoaded={vi.fn()} />)
    expect(screen.getByText('SquireCLI')).toBeInTheDocument()
    expect(screen.getByText('AI-powered code assistant')).toBeInTheDocument()
  })

  it('calls onLoaded after animation completes', () => {
    const onLoaded = vi.fn()
    render(<SplashScreen onLoaded={onLoaded} />)

    act(() => {
      vi.advanceTimersByTime(800)
    })

    expect(onLoaded).not.toHaveBeenCalled()

    act(() => {
      vi.advanceTimersByTime(300)
    })

    expect(onLoaded).toHaveBeenCalledTimes(1)
  })

  it('shows loading bar', () => {
    render(<SplashScreen onLoaded={vi.fn()} />)
    const loadingBar = document.querySelector('.bg-primary.rounded-full')
    expect(loadingBar).toBeInTheDocument()
  })
})
