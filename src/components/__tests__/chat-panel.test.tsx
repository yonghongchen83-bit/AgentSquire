import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ChatPanel } from '@/components/chat-panel'
import { useChatStore } from '@/stores/chat-store'
import type { SessionSummary } from '@/types/ipc'

// chat-panel.tsx calls loadConversations()/loadProviders() on mount and reads config for
// the MCP tab count — mock the IPC boundary so mount doesn't attempt a real Tauri call.
vi.mock('@/lib/ipc', () => ({
  loadConfig: vi.fn().mockResolvedValue({ mcpServers: [] }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))

const legacyConversation: SessionSummary = {
  id: 'legacy-1',
  title: 'Legacy Chat',
  messageCount: 1,
  lastMessageAt: new Date().toISOString(),
  createdAt: new Date().toISOString(),
  contextMode: 'legacy',
}

const squireConversation: SessionSummary = {
  id: 'squire-1',
  title: 'Squire Chat',
  messageCount: 1,
  lastMessageAt: new Date().toISOString(),
  createdAt: new Date().toISOString(),
  contextMode: 'squire',
}

// session-ux-polish: the active conversation's chat header should show a small "Squire"
// badge when (and only when) the open conversation is in Squire mode — matching the
// sidebar row badge's own "badge only for the non-default case" convention exactly (see
// decisions.md).
describe('ChatPanel mode indicator', () => {
  beforeEach(() => {
    useChatStore.setState({
      conversations: [legacyConversation, squireConversation],
      activeConversationId: null,
      messages: [],
      isStreaming: false,
      streamingBlocks: [],
      streamingStatus: '',
      pendingApprovals: [],
      pendingAskUserQuestion: null,
      error: null,
      providers: [],
      autoApproveScope: 'none',
    })
  })

  it('shows no Squire badge in the chat header when no conversation is active', () => {
    render(<ChatPanel />)
    expect(screen.queryByTitle("This session uses Squire's curated protocol context")).not.toBeInTheDocument()
  })

  it('shows no Squire badge in the chat header for a legacy-mode conversation', () => {
    useChatStore.setState({ activeConversationId: 'legacy-1' })
    render(<ChatPanel />)
    expect(screen.queryByTitle("This session uses Squire's curated protocol context")).not.toBeInTheDocument()
  })

  it('shows the Squire badge in the chat header for a squire-mode conversation', () => {
    useChatStore.setState({ activeConversationId: 'squire-1' })
    render(<ChatPanel />)
    expect(screen.getByTitle("This session uses Squire's curated protocol context")).toBeInTheDocument()
  })
})
