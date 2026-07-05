import { useState, useCallback } from 'react'
import type { Block } from '@/types/ipc'
import { ChevronDown, ChevronRight, Wrench, Check, X, AlertCircle, Copy, FileDown, Diff, Bot, ExternalLink, Loader2, Circle, Clock, CircleCheck } from 'lucide-react'
import { useChatStore } from '@/stores/chat-store'
import { useSubagentStore } from '@/stores/subagent-store'

function TextBlock({ content }: { content: string }) {
  return <p className="whitespace-pre-wrap text-sm leading-relaxed">{content}</p>
}

function ThinkingBlock({ content }: { content: string }) {
  const [expanded, setExpanded] = useState(true)
  return (
    <div className="border border-border rounded-md overflow-hidden my-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium text-[#6B7B8D] bg-muted hover:bg-[#E8EDF2] transition-colors"
      >
        {expanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        <span>Thinking</span>
      </button>
      {expanded && (
        <div className="px-3 py-2 text-sm text-[#6B7B8D] italic whitespace-pre-wrap bg-[#F8F9FB]">
          {content}
        </div>
      )}
    </div>
  )
}

// ── Todo Tree Viewer ──

interface TodoTreeItem {
  id: string
  title: string
  status: 'todo' | 'in_progress' | 'done'
  children: TodoTreeItem[]
}

function isTodoTreePayload(result: string): { items: TodoTreeItem[] } | null {
  try {
    const parsed = JSON.parse(result)
    if (parsed._type === 'todo_tree' && Array.isArray(parsed.items)) {
      return { items: parsed.items }
    }
    return null
  } catch {
    return null
  }
}

function parseTodoOperation(args: string): string | null {
  try {
    const parsed = JSON.parse(args)
    if (typeof parsed.operation === 'string') return parsed.operation
    return null
  } catch {
    return null
  }
}

function isTreeProducingOperation(operation: string | null): boolean {
  return operation === 'list' || operation === 'get'
}

function TodoTreeItemRow({ item, depth }: { item: TodoTreeItem; depth: number }) {
  const [expanded, setExpanded] = useState(true)
  const children = item.children ?? []
  const hasChildren = children.length > 0

  const statusIcon = item.status === 'done'
    ? <CircleCheck className="h-4 w-4 text-green-500 flex-shrink-0" />
    : item.status === 'in_progress'
      ? <Clock className="h-4 w-4 text-amber-500 flex-shrink-0" />
      : <Circle className="h-4 w-4 text-gray-400 flex-shrink-0" />

  return (
    <div>
      <div
        className="flex items-center gap-1.5 py-1 text-sm cursor-pointer hover:bg-gray-50 rounded px-1"
        style={{ paddingLeft: `${depth * 20 + 4}px` }}
        onClick={() => hasChildren && setExpanded(!expanded)}
      >
        {hasChildren ? (
          expanded
            ? <ChevronDown className="h-3.5 w-3.5 text-gray-400 flex-shrink-0" />
            : <ChevronRight className="h-3.5 w-3.5 text-gray-400 flex-shrink-0" />
        ) : (
          <span className="w-3.5 flex-shrink-0" />
        )}
        {statusIcon}
        <span className={item.status === 'done' ? 'line-through text-gray-500' : 'text-gray-800'}>
          {item.title}
        </span>
        <span className="text-[10px] text-gray-400 ml-1">
          {item.status === 'todo' ? 'todo' : item.status === 'in_progress' ? 'wip' : 'done'}
        </span>
      </div>
      {expanded && hasChildren && (
        <div>
          {children.map(child => (
            <TodoTreeItemRow key={child.id} item={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  )
}

function TodoTreeViewer({ items }: { items: TodoTreeItem[] }) {
  if (items.length === 0) {
    return <p className="text-sm text-gray-500 py-2">No todo items found.</p>
  }
  return (
    <div className="py-1">
      {items.map(item => (
        <TodoTreeItemRow key={item.id} item={item} depth={0} />
      ))}
    </div>
  )
}

function ToolCallBlock({ block }: { block: Extract<Block, { type: 'tool_call' }> }) {
  const approveToolCall = useChatStore((s) => s.approveToolCall)
  const rejectToolCall = useChatStore((s) => s.rejectToolCall)

  // For run_terminal: build a nice display label
  const isTerminalTool = block.toolName === 'run_terminal'
  const isTodoTree = block.toolName === 'todo_tree'
  const cmdAnalysis = block.commandAnalysis
  const displayLabel = isTerminalTool && cmdAnalysis
    ? `${cmdAnalysis.command} ${cmdAnalysis.args.join(' ')}`
    : null

  const hasOutsidePaths = cmdAnalysis?.paths?.some(p => p.isOutsideWorkspace)

  // Parse todo tree payload if applicable
  const treeData = isTodoTree && block.result ? isTodoTreePayload(block.result) : null
  const todoOperation = isTodoTree ? parseTodoOperation(block.args) : null
  const isTreeOperation = isTreeProducingOperation(todoOperation)

  // Only render todo_tree blocks that produce a tree (list/get).
  // Create/update/delete are administrative — don't show them in chat.
  if (isTodoTree && !isTreeOperation) return null

  const [expanded, setExpanded] = useState(isTreeOperation)

  return (
    <div className="border border-border rounded-md overflow-hidden my-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium text-[#6B7B8D] bg-muted hover:bg-[#E8EDF2] transition-colors"
      >
        <Wrench className="h-3 w-3 flex-shrink-0" />
        <span className="flex-1 min-w-0 text-left">
          {isTodoTree ? (
            <>Todo Tree</>
          ) : displayLabel ? (
            <>
              <span className="font-mono text-[11px]">{displayLabel}</span>
              {hasOutsidePaths && (
                <span className="ml-2 inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-orange-100 text-orange-700">
                  <AlertCircle className="h-2.5 w-2.5" />
                  Outside workspace
                </span>
              )}
            </>
          ) : (
            <>Tool: {block.toolName}</>
          )}
          {block.isPending && (
            <span className="ml-2 inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-amber-100 text-amber-700">
              <AlertCircle className="h-2.5 w-2.5" />
              Pending approval
            </span>
          )}
          {block.isError && block.result && (
            <span className="ml-2 inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-red-100 text-red-700">
              Error
            </span>
          )}
        </span>
        {expanded ? <ChevronDown className="h-3 w-3 ml-auto flex-shrink-0" /> : <ChevronRight className="h-3 w-3 ml-auto flex-shrink-0" />}
      </button>
      {expanded && (
        <div className={treeData ? 'px-3 py-1 bg-[#F8F9FB]' : 'px-3 py-2 text-sm font-mono whitespace-pre-wrap bg-[#F8F9FB] max-h-48 overflow-auto'}>
          {/* Command info summary for terminal tools */}
          {cmdAnalysis && cmdAnalysis.paths.length > 0 && (
            <div className="mb-2 pb-2 border-b border-border text-xs">
              <div className="font-semibold text-[#6B7B8D] mb-1">Paths:</div>
              {cmdAnalysis.paths.map((p, i) => (
                <div key={i} className={`flex items-center gap-1.5 py-0.5 ${p.isOutsideWorkspace ? 'text-orange-600' : ''}`}>
                  {p.isOutsideWorkspace ? <AlertCircle className="h-3 w-3 flex-shrink-0" /> : <span className="w-3" />}
                  <span className="truncate">{p.original}</span>
                  {p.isOutsideWorkspace && (
                    <span className="text-[10px] font-medium text-orange-600 ml-auto flex-shrink-0">outside workspace</span>
                  )}
                </div>
              ))}
            </div>
          )}
          {treeData ? (
            <TodoTreeViewer items={treeData.items} />
          ) : (
            <>
              {block.args}
              {block.result && (
                <div className={`mt-2 pt-2 border-t ${block.isError ? 'border-red-200' : 'border-border'}`}>
                  <div className={`text-xs font-semibold mb-1 ${block.isError ? 'text-red-600' : 'text-[#6B7B8D]'}`}>
                    {block.isError ? 'Error:' : 'Result:'}
                  </div>
                  <pre className={`text-xs ${block.isError ? 'text-red-600' : ''}`}>{block.result}</pre>
                </div>
              )}
            </>
          )}
        </div>
      )}
      {block.isPending && block.callId && (
        <div className="flex flex-col gap-2 px-3 py-2 bg-amber-50 border-t border-amber-200">
          {/* Rich approval text */}
          <span className="text-xs text-amber-700">
            {isTerminalTool && displayLabel
              ? `Allow this command to run?`
              : `This tool modifies files or runs commands. Approve?`}
          </span>
          {hasOutsidePaths && (
            <span className="text-[11px] text-orange-600 flex items-center gap-1">
              <AlertCircle className="h-3 w-3" />
              Some paths are outside your workspace — review before approving
            </span>
          )}
          <div className="flex items-center gap-2">
            <button
              onClick={(e) => {
                e.stopPropagation()
                approveToolCall(block.callId!)
              }}
              className="inline-flex items-center gap-1 px-2 py-1 text-xs font-medium text-white bg-green-600 hover:bg-green-700 rounded transition-colors"
            >
              <Check className="h-3 w-3" />
              Approve
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation()
                rejectToolCall(block.callId!)
              }}
              className="inline-flex items-center gap-1 px-2 py-1 text-xs font-medium text-white bg-red-600 hover:bg-red-700 rounded transition-colors"
            >
              <X className="h-3 w-3" />
              Reject
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

function SubagentBlockRenderer({ block }: { block: Extract<Block, { type: 'subagent' }> }) {
  const setActiveTab = useSubagentStore((s) => s.setActiveTab)
  const isRunning = block.status === 'running'

  const handleOpen = () => {
    setActiveTab(block.sessionId)
  }

  return (
    <div className={`border rounded-md overflow-hidden my-1 ${
      isRunning ? 'border-[#4A90D9]' : block.status === 'error' ? 'border-red-300' : 'border-border'
    }`}>
      <div className="flex items-center gap-2 px-3 py-2 text-xs font-medium bg-muted">
        {isRunning ? (
          <Loader2 className="h-3.5 w-3.5 text-[#4A90D9] animate-spin flex-shrink-0" />
        ) : (
          <Bot className="h-3.5 w-3.5 text-[#6B7B8D] flex-shrink-0" />
        )}
        <span className="flex-1 truncate">
          {isRunning ? 'Subagent working...' : `Subagent${block.status === 'error' ? ' (error)' : ''}`}
        </span>
        <span className="text-[10px] text-[#6B7B8D] truncate max-w-[200px]">{block.task}</span>
        <button
          onClick={handleOpen}
          className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-[#D0DCE8] text-[#4A90D9] transition-colors flex-shrink-0"
          title="Open subagent chat"
        >
          <ExternalLink className="h-3 w-3" />
          <span>Open</span>
        </button>
      </div>
      {!isRunning && block.result && (
        <div className="px-3 py-2 text-sm text-[#6B7B8D] whitespace-pre-wrap bg-[#F8F9FB] max-h-32 overflow-auto border-t border-border">
          {block.result.length > 300 ? block.result.slice(0, 300) + '...' : block.result}
        </div>
      )}
    </div>
  )
}

function CodeBlock({ block }: { block: Extract<Block, { type: 'code' }> }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(block.content).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [block.content])

  return (
    <div className="border border-border rounded-md overflow-hidden my-1">
      <div className="flex items-center justify-between px-3 py-1.5 text-xs text-[#6B7B8D] bg-muted">
        <span>{block.language || 'code'}</span>
        <div className="flex items-center gap-1">
          <button
            onClick={handleCopy}
            className="flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-[#D0DCE8] transition-colors"
            title={copied ? 'Copied!' : 'Copy'}
          >
            {copied ? <Check className="h-3 w-3 text-green-600" /> : <Copy className="h-3 w-3" />}
            <span>{copied ? 'Copied' : 'Copy'}</span>
          </button>
          <button
            className="flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-[#D0DCE8] transition-colors"
            title="Apply to file (not yet implemented)"
          >
            <FileDown className="h-3 w-3" />
            <span>Apply</span>
          </button>
          <button
            className="flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-[#D0DCE8] transition-colors"
            title="Diff view (not yet implemented)"
          >
            <Diff className="h-3 w-3" />
            <span>Diff</span>
          </button>
        </div>
      </div>
      <pre className="p-3 text-sm font-mono overflow-x-auto bg-[#1A2332] text-[#E8EDF2]">
        <code>{block.content}</code>
      </pre>
    </div>
  )
}

export function ChatBlocks({ blocks }: { blocks: Block[] }) {
  if (blocks.length === 0) return null
  return (
    <div className="space-y-1">
      {blocks.map((block, i) => {
        switch (block.type) {
          case 'text':
            return <TextBlock key={i} content={block.content} />
          case 'thinking':
            return <ThinkingBlock key={i} content={block.content} />
          case 'tool_call':
            return <ToolCallBlock key={i} block={block} />
          case 'code':
            return <CodeBlock key={i} block={block} />
          case 'subagent':
            return <SubagentBlockRenderer key={i} block={block} />
        }
      })}
    </div>
  )
}
