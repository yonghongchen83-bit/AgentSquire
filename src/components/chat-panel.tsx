import { useEffect, useMemo, useCallback, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useChatStore } from '@/stores/chat-store'
import { loadConfig } from '@/lib/ipc'
import { ChatMessage } from '@/components/chat-message'
import { ChatInput } from '@/components/chat-input'
import { ConversationSidebar } from '@/components/conversation-sidebar'
import { MessagesSquare, MessageSquareText, PlugZap, AlertCircle, Check, X } from 'lucide-react'
import {
  Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { McpPanel } from '@/components/mcp-panel'

export function ChatPanel() {
  const [activeTab, setActiveTab] = useState<'chat' | 'conversations' | 'mcp'>('chat')
  const [activeMcpCount, setActiveMcpCount] = useState(0)
  const [takingLong, setTakingLong] = useState(false)
  const conversations = useChatStore((s) => s.conversations)
  const activeConversationId = useChatStore((s) => s.activeConversationId)
  const messages = useChatStore((s) => s.messages)
  const isStreaming = useChatStore((s) => s.isStreaming)
  const streamingBlocks = useChatStore((s) => s.streamingBlocks)
  const streamingStatus = useChatStore((s) => s.streamingStatus)
  const pendingApprovals = useChatStore((s) => s.pendingApprovals)
  const error = useChatStore((s) => s.error)
  const providers = useChatStore((s) => s.providers)
  const selectedProvider = useChatStore((s) => s.selectedProvider)
  const selectedModel = useChatStore((s) => s.selectedModel)
  const selectedThinkingLevel = useChatStore((s) => s.selectedThinkingLevel)
  const loadConversations = useChatStore((s) => s.loadConversations)
  const loadProviders = useChatStore((s) => s.loadProviders)
  const selectConversation = useChatStore((s) => s.selectConversation)
  const createNewConversation = useChatStore((s) => s.createNewConversation)
  const renameConversation = useChatStore((s) => s.renameConversation)
  const deleteConversation = useChatStore((s) => s.deleteConversation)
  const setSelectedProvider = useChatStore((s) => s.setSelectedProvider)
  const setSelectedModel = useChatStore((s) => s.setSelectedModel)
  const setSelectedThinkingLevel = useChatStore((s) => s.setSelectedThinkingLevel)
  const sendMessage = useChatStore((s) => s.sendMessage)
  const cancelStreaming = useChatStore((s) => s.cancelStreaming)
  const approveToolCall = useChatStore((s) => s.approveToolCall)
  const rejectToolCall = useChatStore((s) => s.rejectToolCall)
  const clearError = useChatStore((s) => s.clearError)
  const truncateMessagesFrom = useChatStore((s) => s.truncateMessagesFrom)
  const retryLastMessage = useChatStore((s) => s.retryLastMessage)
  const approveAllPending = useChatStore((s) => s.approveAllPending)
  const autoApproveScope = useChatStore((s) => s.autoApproveScope)
  const setAutoApproveScope = useChatStore((s) => s.setAutoApproveScope)

  useEffect(() => {
    loadConversations()
    loadProviders()
    const unlisten = listen('providers-changed', () => {
      loadProviders()
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [loadConversations, loadProviders])

  useEffect(() => {
    const refreshMcpCount = async () => {
      try {
        const cfg = await loadConfig()
        const active = (cfg.mcpServers ?? []).filter((s) => s.enabled).length
        setActiveMcpCount(active)
      } catch {
        setActiveMcpCount(0)
      }
    }

    refreshMcpCount()
    const onFocus = () => { refreshMcpCount() }
    window.addEventListener('focus', onFocus)

    return () => {
      window.removeEventListener('focus', onFocus)
    }
  }, [])

  useEffect(() => {
    if (!isStreaming) {
      setTakingLong(false)
      return
    }
    setTakingLong(false)
    const timer = window.setTimeout(() => {
      setTakingLong(true)
    }, 12000)
    return () => {
      window.clearTimeout(timer)
    }
  }, [isStreaming, streamingStatus])

  const handleModelSelect = useCallback((value: string) => {
    const [providerName, modelName] = value.split('::')
    if (providerName && modelName) {
      setSelectedProvider(providerName)
      setSelectedModel(modelName)
    }
  }, [setSelectedProvider, setSelectedModel])

  const currentLabel = useMemo(() => {
    if (selectedProvider && selectedModel) {
      return `${selectedProvider} · ${selectedModel}`
    }
    return 'Select model...'
  }, [selectedProvider, selectedModel])

  const currentValue = useMemo(() => {
    if (selectedProvider && selectedModel) {
      return `${selectedProvider}::${selectedModel}`
    }
    return ''
  }, [selectedProvider, selectedModel])

  const handleSelectConversation = useCallback(async (id: string) => {
    await selectConversation(id)
    setActiveTab('chat')
  }, [selectConversation])

  return (
    <div className="flex h-full bg-background">
      <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as 'chat' | 'conversations' | 'mcp')} className="flex h-full w-full">
        <div className="flex-1 min-w-0 flex flex-col">
          {error && (
            <div className="flex items-center gap-2 px-4 py-2 bg-destructive/10 text-destructive text-sm border-b border-border">
              <span className="flex-1">{error}</span>
              <button
                onClick={clearError}
                className="text-xs hover:underline"
              >
                Dismiss
              </button>
            </div>
          )}

          <TabsContent value="chat" className="mt-0 flex-1 flex min-h-0 flex-col">
            {providers.length > 0 && (
              <div className="flex items-center gap-2 px-4 py-1.5 border-b border-border">
                <span className="text-xs text-muted-foreground shrink-0">Model:</span>
                <Select value={currentValue} onValueChange={handleModelSelect}>
                  <SelectTrigger className="h-7 text-xs w-auto min-w-[200px]">
                    <SelectValue placeholder="Select model...">{currentLabel}</SelectValue>
                  </SelectTrigger>
                  <SelectContent className="max-h-[300px]">
                    {providers.map((prov) => (
                      <SelectGroup key={prov.name}>
                        <SelectLabel className="text-xs font-semibold text-muted-foreground px-2 py-1">
                          {prov.name}
                        </SelectLabel>
                        {prov.models.map((m) => (
                          <SelectItem
                            key={`${prov.name}::${m}`}
                            value={`${prov.name}::${m}`}
                            className="text-xs pl-6"
                          >
                            {m}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    ))}
                  </SelectContent>
                </Select>
                <span className="text-xs text-muted-foreground shrink-0 ml-3">Thinking:</span>
                <Select
                  value={selectedThinkingLevel}
                  onValueChange={(v) => setSelectedThinkingLevel(v as 'none' | 'low' | 'mid' | 'high')}
                >
                  <SelectTrigger className="h-7 text-xs w-[110px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none">None</SelectItem>
                    <SelectItem value="low">Low</SelectItem>
                    <SelectItem value="mid">Mid</SelectItem>
                    <SelectItem value="high">High</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            )}
            <div className="flex-1 overflow-y-auto">
              {messages.length === 0 && !isStreaming ? (
                <div className="flex flex-col items-center justify-center h-full gap-3 text-[#6B7B8D] p-6">
                  <MessagesSquare className="h-12 w-12" />
                  <h2 className="text-lg font-semibold">Chat</h2>
                  <p className="text-sm text-center max-w-md">
                    Ask questions about your codebase, request changes, or get help with tasks.
                  </p>
                </div>
              ) : (
                <div className="divide-y divide-border">
                  {messages.map((msg, idx) => {
                    const isLastMessage = idx === messages.length - 1 && !isStreaming
                    return (
                      <ChatMessage
                        key={msg.id}
                        message={msg}
                        augmentBlocks={msg.blocks}
                        isLastMessage={isLastMessage}
                        onRetry={isLastMessage ? retryLastMessage : undefined}
                        onDelete={msg.role === 'user' ? () => truncateMessagesFrom(msg.id) : undefined}
                      />
                    )
                  })}
                  {isStreaming && (
                    <ChatMessage
                      message={{
                        id: 'streaming',
                        sessionId: activeConversationId || '',
                        role: 'assistant',
                        content: '',
                        createdAt: new Date().toISOString(),
                      }}
                      streamingBlocks={streamingBlocks}
                      isStreaming
                    />
                  )}
                </div>
              )}
            </div>
            {(isStreaming || pendingApprovals.length > 0 || autoApproveScope !== 'none') && (
              <div className="border-t border-border bg-[#F8F9FB] px-4 py-2 space-y-2">
                {isStreaming && (
                  <div className="flex items-center gap-2 text-xs text-[#6B7B8D]">
                    <AlertCircle className="h-3.5 w-3.5" />
                    <span className="flex-1">
                      {streamingStatus || 'Working...'}
                    </span>
                    {takingLong && (
                      <button
                        onClick={cancelStreaming}
                        className="px-2 py-0.5 rounded bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors"
                      >
                        Stop
                      </button>
                    )}
                  </div>
                )}
                {takingLong && isStreaming && (
                  <div className="text-[11px] text-amber-700">
                    This is taking longer than usual. You can keep waiting or stop it now.
                  </div>
                )}
                {pendingApprovals.length > 0 && (
                  <div className="flex items-center gap-2 pb-1">
                    <span className="text-xs text-amber-700 flex-1 font-medium">
                      {pendingApprovals.length} tool{pendingApprovals.length > 1 ? 's' : ''} awaiting approval
                    </span>
                    <button
                      onClick={() => { setAutoApproveScope('session'); void approveAllPending() }}
                      className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium text-white bg-green-600 hover:bg-green-700 rounded"
                    >
                      <Check className="h-3 w-3" />
                      Approve All
                    </button>
                    <button
                      onClick={() => { setAutoApproveScope('session'); void approveAllPending() }}
                      className={`px-2 py-0.5 text-xs rounded border transition-colors ${autoApproveScope === 'session' ? 'bg-green-100 border-green-400 text-green-800' : 'border-border text-[#6B7B8D] hover:bg-muted'}`}
                      title="Auto-approve all tools for this conversation"
                    >
                      Auto: Session
                    </button>
                    <button
                      onClick={() => { setAutoApproveScope('workspace'); void approveAllPending() }}
                      className={`px-2 py-0.5 text-xs rounded border transition-colors ${autoApproveScope === 'workspace' ? 'bg-green-100 border-green-400 text-green-800' : 'border-border text-[#6B7B8D] hover:bg-muted'}`}
                      title="Auto-approve all tools for all conversations"
                    >
                      Auto: Workspace
                    </button>
                  </div>
                )}
                {pendingApprovals.map((approval) => (
                  <div
                    key={approval.call_id}
                    className="flex items-center gap-2 rounded border border-amber-200 bg-amber-50 px-2 py-1.5"
                  >
                    <span className="text-xs text-amber-800 flex-1 truncate">
                      Approval required: {approval.tool_name}
                    </span>
                    <button
                      onClick={() => approveToolCall(approval.call_id)}
                      className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium text-white bg-green-600 hover:bg-green-700 rounded"
                    >
                      <Check className="h-3 w-3" />
                      Approve
                    </button>
                    <button
                      onClick={() => rejectToolCall(approval.call_id)}
                      className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium text-white bg-red-600 hover:bg-red-700 rounded"
                    >
                      <X className="h-3 w-3" />
                      Reject
                    </button>
                  </div>
                ))}
                {autoApproveScope !== 'none' && (
                  <div className="flex items-center gap-2 text-xs text-green-700 bg-green-50 rounded px-2 py-1">
                    <Check className="h-3 w-3" />
                    <span className="flex-1">Auto-approving tools ({autoApproveScope})</span>
                    <button onClick={() => setAutoApproveScope('none')} className="hover:underline">
                      Disable
                    </button>
                  </div>
                )}
              </div>
            )}
            <ChatInput
              onSend={sendMessage}
              onCancel={cancelStreaming}
              isStreaming={isStreaming}
            />
          </TabsContent>

          <TabsContent value="conversations" className="mt-0 flex-1 min-h-0 overflow-hidden">
            <ConversationSidebar
              conversations={conversations}
              activeId={activeConversationId}
              onSelect={handleSelectConversation}
              onCreate={createNewConversation}
              onRename={renameConversation}
              onDelete={deleteConversation}
              standalone
            />
          </TabsContent>

          <TabsContent value="mcp" className="mt-0 flex-1 min-h-0 overflow-hidden">
            <McpPanel />
          </TabsContent>
        </div>

        <div className="w-16 border-l border-border bg-[#E8EDF2]">
          <TabsList className="h-full w-full flex-col justify-start gap-1 rounded-none bg-transparent p-1">
            <TabsTrigger value="chat" className="h-14 w-full p-1 text-[11px] leading-tight">
              <span className="flex flex-col items-center gap-1">
                <MessagesSquare className="h-4 w-4" />
                Chat
              </span>
            </TabsTrigger>
            <TabsTrigger value="conversations" className="h-14 w-full p-1 text-[11px] leading-tight">
              <span className="flex flex-col items-center gap-1">
                <MessageSquareText className="h-4 w-4" />
                <span>Sessions</span>
                <span className="text-[10px] text-[#6B7B8D]">{conversations.length}</span>
              </span>
            </TabsTrigger>
            <TabsTrigger value="mcp" className="h-14 w-full p-1 text-[11px] leading-tight">
              <span className="flex flex-col items-center gap-1">
                <PlugZap className="h-4 w-4" />
                <span>MCP</span>
                <span className="text-[10px] text-[#6B7B8D]">{activeMcpCount}</span>
              </span>
            </TabsTrigger>
          </TabsList>
        </div>
      </Tabs>
    </div>
  )
}