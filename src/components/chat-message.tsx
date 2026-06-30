import type { Message, Block } from '@/types/ipc'
import { ChatBlocks } from '@/components/chat-blocks'
import { User, Bot, Copy, Check, RotateCcw, Trash2 } from 'lucide-react'
import { useState } from 'react'

function parseBlocks(content: string): Block[] {
  const thinkingRegex = /<thinking>\s*([\s\S]*?)\s*<\/thinking>/i
  const thinkingMatch = thinkingRegex.exec(content)

  let cleaned = content
  const blocks: Block[] = []

  if (thinkingMatch) {
    const thinking = thinkingMatch[1].trim()
    if (thinking) {
      blocks.push({ type: 'thinking', content: thinking })
    }
    cleaned = content.replace(thinkingRegex, '').trim()
  }

  const parsed = parseTextAndCodeBlocks(cleaned)
  blocks.push(...parsed)
  return blocks
}

function parseTextAndCodeBlocks(content: string): Block[] {
  const blocks: Block[] = []
  let remaining = content
  const codeBlockRegex = /```(\w*)\n?([\s\S]*?)```/g
  let match: RegExpExecArray | null
  let lastIndex = 0

  while ((match = codeBlockRegex.exec(remaining)) !== null) {
    if (match.index > lastIndex) {
      const before = remaining.slice(lastIndex, match.index).trim()
      if (before) blocks.push({ type: 'text', content: before })
    }
    blocks.push({
      type: 'code',
      language: match[1] || 'plaintext',
      content: match[2].trim(),
    })
    lastIndex = match.index + match[0].length
  }

  const after = remaining.slice(lastIndex).trim()
  if (after) blocks.push({ type: 'text', content: after })

  if (blocks.length === 0 && content) {
    blocks.push({ type: 'text', content })
  }

  return blocks
}

interface ChatMessageProps {
  message: Message
  streamingBlocks?: Block[]
  isStreaming?: boolean
  isLastMessage?: boolean
  augmentBlocks?: Block[]
  onRetry?: () => void
  onDelete?: () => void
}

export function ChatMessage({ message, streamingBlocks, isStreaming, isLastMessage, augmentBlocks, onRetry, onDelete }: ChatMessageProps) {
  const isUser = message.role === 'user'
  const blocks = parseBlocks(message.content)
  const [copied, setCopied] = useState(false)

  const handleCopy = () => {
    void navigator.clipboard.writeText(message.content).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }

  return (
    <div className={`group relative flex gap-3 px-4 py-3 ${isUser ? '' : 'bg-[#F8F9FB]'}`}>
      <div className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center ${
        isUser ? 'bg-[#4A90D9] text-white' : 'bg-[#1A2332] text-white'
      }`}>
        {isUser ? <User className="h-4 w-4" /> : <Bot className="h-4 w-4" />}
      </div>
      <div className="flex-1 min-w-0 space-y-2">
        <div className="text-xs font-semibold text-[#6B7B8D]">
          {isUser ? 'You' : 'Assistant'}
        </div>
        {augmentBlocks && augmentBlocks.length > 0 && (
          <ChatBlocks blocks={augmentBlocks} />
        )}
        <ChatBlocks blocks={blocks} />
        {isStreaming && streamingBlocks && streamingBlocks.length > 0 && (
          <div className="border-l-2 border-[#4A90D9] pl-3">
            <ChatBlocks blocks={streamingBlocks} />
            <span className="inline-block w-2 h-4 bg-[#4A90D9] animate-pulse ml-1" />
          </div>
        )}
        {isStreaming && !streamingBlocks?.length && (
          <div className="flex items-center gap-1 text-[#6B7B8D]">
            <span className="inline-block w-2 h-4 bg-[#4A90D9] animate-pulse" />
            <span className="text-xs">Thinking...</span>
          </div>
        )}
      </div>
      {!isStreaming && (
        <div className="absolute top-2 right-2 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
          <button
            onClick={handleCopy}
            title="Copy message"
            className="p-1 rounded text-[#6B7B8D] hover:text-foreground hover:bg-black/5 transition-colors"
          >
            {copied ? <Check className="h-3.5 w-3.5 text-green-600" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
          {isLastMessage && onRetry && (
            <button
              onClick={onRetry}
              title="Retry"
              className="p-1 rounded text-[#6B7B8D] hover:text-foreground hover:bg-black/5 transition-colors"
            >
              <RotateCcw className="h-3.5 w-3.5" />
            </button>
          )}
          {isUser && onDelete && (
            <button
              onClick={onDelete}
              title="Delete from here"
              className="p-1 rounded text-[#6B7B8D] hover:text-destructive hover:bg-black/5 transition-colors"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      )}
    </div>
  )
}