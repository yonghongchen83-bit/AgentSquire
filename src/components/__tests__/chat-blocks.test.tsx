import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ChatBlocks } from '@/components/chat-blocks'
import type { Block } from '@/types/ipc'

describe('ChatBlocks', () => {
  it('renders text blocks', () => {
    const blocks: Block[] = [{ type: 'text', content: 'Hello world' }]
    render(<ChatBlocks blocks={blocks} />)
    expect(screen.getByText('Hello world')).toBeInTheDocument()
  })

  it('renders code blocks', () => {
    const blocks: Block[] = [{ type: 'code', language: 'typescript', content: 'const x = 1' }]
    render(<ChatBlocks blocks={blocks} />)
    expect(screen.getByText('const x = 1')).toBeInTheDocument()
    expect(screen.getByText('typescript')).toBeInTheDocument()
  })

  it('renders thinking blocks collapsed by default', () => {
    const blocks: Block[] = [{ type: 'thinking', content: 'I am thinking...' }]
    render(<ChatBlocks blocks={blocks} />)
    expect(screen.getByText('Thinking')).toBeInTheDocument()
    expect(screen.queryByText('I am thinking...')).not.toBeInTheDocument()
  })

  it('renders tool call blocks collapsed by default', () => {
    const blocks: Block[] = [{ type: 'tool_call', toolName: 'read_file', args: '{"path":"/test"}' }]
    render(<ChatBlocks blocks={blocks} />)
    expect(screen.getByText(/Tool: read_file/)).toBeInTheDocument()
    expect(screen.queryByText('{"path":"/test"}')).not.toBeInTheDocument()
  })

  it('renders multiple blocks', () => {
    const blocks: Block[] = [
      { type: 'text', content: 'First' },
      { type: 'text', content: 'Second' },
    ]
    render(<ChatBlocks blocks={blocks} />)
    expect(screen.getByText('First')).toBeInTheDocument()
    expect(screen.getByText('Second')).toBeInTheDocument()
  })

  it('returns null for empty blocks', () => {
    const { container } = render(<ChatBlocks blocks={[]} />)
    expect(container.innerHTML).toBe('')
  })
})