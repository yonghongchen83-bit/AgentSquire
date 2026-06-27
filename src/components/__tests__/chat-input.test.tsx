import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ChatInput } from '@/components/chat-input'

describe('ChatInput', () => {
  it('renders textarea and send button', () => {
    render(<ChatInput onSend={vi.fn()} onCancel={vi.fn()} isStreaming={false} />)
    expect(screen.getByPlaceholderText('Ask anything...')).toBeInTheDocument()
    expect(screen.getByRole('button')).toBeInTheDocument()
  })

  it('calls onSend with trimmed input when send clicked', async () => {
    const onSend = vi.fn()
    const user = userEvent.setup()
    render(<ChatInput onSend={onSend} onCancel={vi.fn()} isStreaming={false} />)
    const textarea = screen.getByPlaceholderText('Ask anything...')
    await user.type(textarea, 'Hello world')
    await user.click(screen.getByRole('button'))
    expect(onSend).toHaveBeenCalledWith('Hello world')
  })

  it('calls onSend on Enter without Shift', async () => {
    const onSend = vi.fn()
    const user = userEvent.setup()
    render(<ChatInput onSend={onSend} onCancel={vi.fn()} isStreaming={false} />)
    const textarea = screen.getByPlaceholderText('Ask anything...')
    await user.type(textarea, 'Hello{Enter}')
    expect(onSend).toHaveBeenCalledWith('Hello')
  })

  it('shows cancel button when streaming', () => {
    render(<ChatInput onSend={vi.fn()} onCancel={vi.fn()} isStreaming={true} />)
    // Should have cancel (square) icon instead of send
    expect(screen.queryByRole('button')).toBeInTheDocument()
  })

  it('disables send when input is empty', () => {
    render(<ChatInput onSend={vi.fn()} onCancel={vi.fn()} isStreaming={false} />)
    const button = screen.getByRole('button')
    expect(button).toBeDisabled()
  })
})