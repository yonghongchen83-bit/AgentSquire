import { describe, expect, it } from 'vitest'
import { composeStreamingBlocks, parseBlocks } from './block-parser'

describe('block-parser', () => {
  it('parses text and fenced code blocks', () => {
    const input = 'hello\n```ts\nconst a = 1\n```\nworld'
    const blocks = parseBlocks(input)

    expect(blocks).toHaveLength(3)
    expect(blocks[0]).toMatchObject({ type: 'text', content: 'hello' })
    expect(blocks[1]).toMatchObject({ type: 'code', language: 'ts', content: 'const a = 1' })
    expect(blocks[2]).toMatchObject({ type: 'text', content: 'world' })
  })

  it('keeps plain text as a text block', () => {
    const blocks = parseBlocks('just text')
    expect(blocks).toEqual([{ type: 'text', content: 'just text' }])
  })

  it('composes thinking + parsed response + existing tool calls', () => {
    const blocks = composeStreamingBlocks('thinking...', 'done', [
      { type: 'tool_call', toolName: 'run_terminal', args: '{}', callId: '1' },
    ])

    expect(blocks[0]).toMatchObject({ type: 'thinking', content: 'thinking...' })
    expect(blocks[1]).toMatchObject({ type: 'text', content: 'done' })
    expect(blocks[2]).toMatchObject({ type: 'tool_call', toolName: 'run_terminal', callId: '1' })
  })
})
