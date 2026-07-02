import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ConversationSidebar } from '@/components/conversation-sidebar'
import type { SessionSummary } from '@/types/ipc'
import { CHAT_SQUIRE_MODE_DEFAULT_KEY } from '@/stores/chat-store/preferences'

const mockConversations: SessionSummary[] = [
  { id: '1', title: 'Chat One', messageCount: 3, lastMessageAt: new Date().toISOString(), createdAt: new Date().toISOString(), contextMode: 'legacy' },
  { id: '2', title: 'Chat Two', messageCount: 1, lastMessageAt: new Date().toISOString(), createdAt: new Date().toISOString(), contextMode: 'squire' },
]

describe('ConversationSidebar', () => {
  beforeEach(() => {
    window.localStorage.clear()
  })

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

  // session-creation-ux: the Squire-mode toggle next to "New session" must default to
  // Legacy (least disruption to existing behavior) and only pass 'squire' to onCreate
  // once explicitly switched on.
  it('creates a new session in legacy mode by default', async () => {
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
    expect(onCreate).toHaveBeenCalledWith('legacy')
  })

  it('creates a new session in squire mode once the toggle is switched on', async () => {
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
    await user.click(screen.getByLabelText('Create new sessions in Squire mode'))
    await user.click(screen.getByTitle('New session'))
    expect(onCreate).toHaveBeenCalledWith('squire')
  })

  it('shows a Squire badge only for sessions in squire mode', () => {
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
    // mockConversations: 'Chat One' is legacy, 'Chat Two' is squire.
    expect(screen.getByTitle("This session uses Squire's curated protocol context")).toBeInTheDocument()
    const chatOneRow = screen.getByText('Chat One').closest('div.group')
    expect(chatOneRow?.textContent).not.toContain('Squire')
  })

  // session-ux-polish: the creation-time toggle's last-chosen value should persist across
  // a remount (e.g. app restart) instead of always resetting to Legacy.
  it('initializes the toggle from a previously stored squire-mode preference', () => {
    window.localStorage.setItem(CHAT_SQUIRE_MODE_DEFAULT_KEY, 'true')
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
    expect(screen.getByLabelText('Create new sessions in Squire mode')).toHaveAttribute('data-state', 'checked')
  })

  it('persists the toggle value to localStorage when switched on', async () => {
    const user = userEvent.setup()
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
    await user.click(screen.getByLabelText('Create new sessions in Squire mode'))
    expect(window.localStorage.getItem(CHAT_SQUIRE_MODE_DEFAULT_KEY)).toBe('true')
  })
})