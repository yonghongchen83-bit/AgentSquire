import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ErrorBoundary } from '@/components/error-boundary'

function ExplodingComponent(): React.ReactNode {
  throw new Error('Boom!')
}

describe('ErrorBoundary', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('renders children when no error', () => {
    render(
      <ErrorBoundary>
        <div>All good</div>
      </ErrorBoundary>,
    )
    expect(screen.getByText('All good')).toBeInTheDocument()
  })

  it('catches errors and shows fallback', () => {
    render(
      <ErrorBoundary>
        <ExplodingComponent />
      </ErrorBoundary>,
    )
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    expect(screen.getByText('Boom!')).toBeInTheDocument()
  })

  it('shows custom fallback when provided', () => {
    render(
      <ErrorBoundary fallback={<div>Custom error</div>}>
        <ExplodingComponent />
      </ErrorBoundary>,
    )
    expect(screen.getByText('Custom error')).toBeInTheDocument()
    expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument()
  })

  it('renders try again button', () => {
    render(
      <ErrorBoundary>
        <ExplodingComponent />
      </ErrorBoundary>,
    )
    expect(screen.getByText('Try again')).toBeInTheDocument()
  })
})
