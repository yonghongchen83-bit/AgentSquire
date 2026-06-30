import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useChatStore } from './chat-store'

let streamDoneHandler: (() => void) | null = null
let streamErrorHandler: ((error: string) => void) | null = null

vi.mock('@/lib/ipc', () => ({
  listConversations: vi.fn().mockResolvedValue([
    { id: '1', title: 'Chat 1', messageCount: 2, lastMessageAt: new Date().toISOString(), createdAt: new Date().toISOString() },
    { id: '2', title: 'Chat 2', messageCount: 5, lastMessageAt: new Date().toISOString(), createdAt: new Date().toISOString() },
  ]),
  getConversation: vi.fn().mockImplementation((id: string) => {
    if (id === '1') {
      return Promise.resolve({
        session: { id: '1', title: 'Chat 1', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() },
        messages: [
          { id: 'm1', sessionId: '1', role: 'user', content: 'Hello', createdAt: new Date().toISOString() },
          { id: 'm2', sessionId: '1', role: 'assistant', content: 'Hi there!', createdAt: new Date().toISOString() },
        ],
      })
    }
    return Promise.reject(new Error('Not found'))
  }),
  createConversation: vi.fn().mockResolvedValue({ id: 'new-id', title: 'New Chat', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }),
  deleteConversation: vi.fn().mockResolvedValue(undefined),
  sendMessage: vi.fn().mockResolvedValue(undefined),
  abortStream: vi.fn().mockResolvedValue(undefined),
  listProviders: vi.fn().mockResolvedValue([{ name: 'OpenAI', provider_type: 'openai', models: ['gpt-4o'], default_model: 'gpt-4o' }]),
  onStreamThinking: vi.fn().mockReturnValue(vi.fn()),
  onStreamChunk: vi.fn().mockReturnValue(vi.fn()),
  onStreamToolCall: vi.fn().mockReturnValue(vi.fn()),
  onStreamToolResult: vi.fn().mockReturnValue(vi.fn()),
  onStreamToolPending: vi.fn().mockReturnValue(vi.fn()),
  onStreamStatus: vi.fn().mockReturnValue(vi.fn()),
  onStreamDone: vi.fn().mockImplementation(async (cb: () => void) => {
    streamDoneHandler = cb
    return vi.fn()
  }),
  onStreamError: vi.fn().mockImplementation(async (cb: (error: string) => void) => {
    streamErrorHandler = cb
    return vi.fn()
  }),
  approveToolCall: vi.fn().mockResolvedValue(undefined),
  rejectToolCall: vi.fn().mockResolvedValue(undefined),
}))

describe('ChatStore', () => {
  beforeEach(() => {
    streamDoneHandler = null
    streamErrorHandler = null
    useChatStore.setState({
      conversations: [],
      activeConversationId: null,
      messages: [],
      isStreaming: false,
      streamingMessageId: null,
      streamingBlocks: [],
      streamingText: '',
      streamingThinkingText: '',
      streamingStatus: '',
      error: null,
      providers: [],
      selectedProvider: '',
      selectedModel: '',
      selectedThinkingLevel: 'mid',
      pendingApprovals: [],
    })
  })

  it('loads conversations', async () => {
    await useChatStore.getState().loadConversations()
    const conversations = useChatStore.getState().conversations
    expect(conversations).toHaveLength(2)
    expect(conversations[0].title).toBe('Chat 1')
  })

  it('selects a conversation and loads messages', async () => {
    await useChatStore.getState().selectConversation('1')
    const state = useChatStore.getState()
    expect(state.activeConversationId).toBe('1')
    expect(state.messages).toHaveLength(2)
    expect(state.messages[0].role).toBe('user')
    expect(state.messages[1].role).toBe('assistant')
  })

  it('creates a new conversation', async () => {
    const id = await useChatStore.getState().createNewConversation()
    expect(id).toBe('new-id')
    expect(useChatStore.getState().activeConversationId).toBe('new-id')
    expect(useChatStore.getState().messages).toEqual([])
  })

  it('clears error on clearError', () => {
    useChatStore.setState({ error: 'Something went wrong' })
    useChatStore.getState().clearError()
    expect(useChatStore.getState().error).toBeNull()
  })

  it('cancels streaming resets streaming state', () => {
    useChatStore.setState({
      isStreaming: true,
      streamingMessageId: 'abc',
      streamingText: 'hello',
      streamingBlocks: [{ type: 'text', content: 'hello' }],
    })
    useChatStore.getState().cancelStreaming()
    const state = useChatStore.getState()
    expect(state.isStreaming).toBe(false)
    expect(state.streamingMessageId).toBeNull()
    expect(state.streamingText).toBe('')
    expect(state.streamingBlocks).toEqual([])
  })

  it('reloads persisted conversation messages on stream done', async () => {
    await useChatStore.getState().selectConversation('1')
    await useChatStore.getState().sendMessage('Test prompt')
    expect(streamDoneHandler).not.toBeNull()

    streamDoneHandler?.()
    await Promise.resolve()
    await Promise.resolve()

    const state = useChatStore.getState()
    expect(state.isStreaming).toBe(false)
    expect(state.messages).toHaveLength(2)
    expect(state.messages[0].role).toBe('user')
    expect(state.messages[1].role).toBe('assistant')
  })

  it('reloads persisted conversation messages on stream error', async () => {
    await useChatStore.getState().selectConversation('1')
    await useChatStore.getState().sendMessage('Test prompt')
    expect(streamErrorHandler).not.toBeNull()

    streamErrorHandler?.('boom')
    await Promise.resolve()
    await Promise.resolve()

    const state = useChatStore.getState()
    expect(state.isStreaming).toBe(false)
    expect(state.error).toBe('boom')
    expect(state.messages).toHaveLength(2)
    expect(state.messages[0].role).toBe('user')
  })
})