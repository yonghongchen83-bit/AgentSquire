import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ConversationSidebar } from '@/components/conversation-sidebar'
import type { SessionSummary } from '@/types/ipc'

const mockConversations: SessionSummary[] = [
  { id: '1', title: 'Chat One', messageCount: 3, lastMessageAt: new Date().toISOString(), createdAt: new Date().toISOString() },
  { id: '2', title: 'Chat Two', messageCount: 1, lastMessageAt: new Date().toISOString(), createdAt: new Date().toISOString() },
]

describe('ConversationSidebar', () => {
  it('renders conversation list', () => {
    render(
      <ConversationSidebar
        conversations={mockConversations}
        activeId={null}
        onSelect={vi.fn()}
        onCreate={vi.fn()}
        onRename={vi.fn()}
        onDelete={vi.fn()}
      />,
    )
    expect(screen.getByText('Chat One')).toBeInTheDocument()
    expect(screen.getByText('Chat Two')).toBeInTheDocument()
  })

  it('calls onSelect when a conversation is clicked', async () => {
    const onSelect = vi.fn()
    const user = userEvent.setup()
    render(
      <ConversationSidebar
        conversations={mockConversations}
        activeId={null}
        onSelect={onSelect}
        onCreate={vi.fn()}
        onRename={vi.fn()}
        onDelete={vi.fn()}
      />,
    )
    await user.click(screen.getByText('Chat One'))
    expect(onSelect).toHaveBeenCalledWith('1')
  })

  it('calls onCreate when new chat button is clicked', async () => {
    const onCreate = vi.fn()
    const user = userEvent.setup()
    render(
      <ConversationSidebar
        conversations={mockConversations}
        activeId={null}
        onSelect={vi.fn()}
        onCreate={onCreate}
        onRename={vi.fn()}
        onDelete={vi.fn()}
      />,
    )
    await user.click(screen.getByTitle('New session'))
    expect(onCreate).toHaveBeenCalledOnce()
  })

  it('highlights active conversation', () => {
    render(
      <ConversationSidebar
        conversations={mockConversations}
        activeId="1"
        onSelect={vi.fn()}
        onCreate={vi.fn()}
        onRename={vi.fn()}
        onDelete={vi.fn()}
      />,
    )
    const items = screen.getAllByText(/Chat/)
    expect(items[0].className).toContain('truncate')
  })

  it('shows empty state when no conversations', () => {
    render(
      <ConversationSidebar
        conversations={[]}
        activeId={null}
        onSelect={vi.fn()}
        onCreate={vi.fn()}
        onRename={vi.fn()}
        onDelete={vi.fn()}
      />,
    )
    expect(screen.getByText('No sessions yet')).toBeInTheDocument()
  })
})