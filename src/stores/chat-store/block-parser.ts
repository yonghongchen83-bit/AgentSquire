import type { Block } from '@/types/ipc'

export function parseBlocks(content: string): Block[] {
  const blocks: Block[] = []
  let remaining = content
  const codeBlockRegex = /```(\w*)\n?([\s\S]*?)```/
  let match: RegExpExecArray | null

  while ((match = codeBlockRegex.exec(remaining)) !== null) {
    if (match.index > 0) {
      const before = remaining.slice(0, match.index).trim()
      if (before) blocks.push({ type: 'text', content: before })
    }
    blocks.push({
      type: 'code',
      language: match[1] || 'plaintext',
      content: match[2].trim(),
    })
    remaining = remaining.slice(match.index + match[0].length)
  }

  const after = remaining.trim()
  if (after) blocks.push({ type: 'text', content: after })

  if (blocks.length === 0 && content) {
    blocks.push({ type: 'text', content })
  }

  return blocks
}

export function composeStreamingBlocks(
  thinkingText: string,
  responseText: string,
  existing: Block[],
): Block[] {
  const nonTextBlocks = existing.filter((b) => b.type === 'tool_call')
  const blocks: Block[] = []

  if (thinkingText.trim()) {
    blocks.push({ type: 'thinking', content: thinkingText })
  }

  blocks.push(...parseBlocks(responseText))
  blocks.push(...nonTextBlocks)
  return blocks
}
