import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ChatBlocks } from '@/components/chat-blocks'
import type { Block } from '@/types/ipc'

const todoTreeResult = JSON.stringify({
  _type: 'todo_tree',
  items: [
    { id: '1', title: 'Root', status: 'todo', children: [
      { id: '2', title: 'Child', status: 'in_progress', children: [
        { id: '3', title: 'Grandchild', status: 'done', children: [] }
      ]}
    ]},
    { id: '4', title: 'Orphan', status: 'done', children: [] }
  ]
})

describe('Todo Tree Tool Block Rendering', () => {
  it('renders todo tree with nested children', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"list"}',
      callId: 'call-1',
      result: todoTreeResult,
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.getByText('Todo Tree')).toBeInTheDocument()
    expect(screen.getByText('Root')).toBeInTheDocument()
    expect(screen.getByText('Child')).toBeInTheDocument()
    expect(screen.getByText('Grandchild')).toBeInTheDocument()
    expect(screen.getByText('Orphan')).toBeInTheDocument()
  })

  it('renders empty todo tree gracefully', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"list"}',
      callId: 'call-2',
      result: JSON.stringify({ _type: 'todo_tree', items: [] }),
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.getByText('No todo items found.')).toBeInTheDocument()
  })

  it('handles todo tree with malformed children gracefully', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"list"}',
      callId: 'call-3',
      result: JSON.stringify({
        _type: 'todo_tree',
        items: [
          { id: '1', title: 'Root', status: 'todo' }
        ]
      }),
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.getByText('Root')).toBeInTheDocument()
  })

  it('handles non-todo-tree tool call with result', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'run_terminal',
      args: '{}',
      callId: 'call-4',
      result: 'Script completed successfully',
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.getByText(/Tool: run_terminal/)).toBeInTheDocument()
  })

  it('does not render create operation at all', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"create","title":"New task"}',
      callId: 'call-5',
      result: 'Created todo item: New task (id: abc-123)',
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.queryByText('Todo Tree')).not.toBeInTheDocument()
    expect(screen.queryByText('create')).not.toBeInTheDocument()
    expect(screen.queryByText('New task')).not.toBeInTheDocument()
  })

  it('does not render update operation at all', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"update","id":"abc","status":"done"}',
      callId: 'call-6',
      result: "Updated 'Task' (abc) to done",
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.queryByText('Todo Tree')).not.toBeInTheDocument()
    expect(screen.queryByText('update')).not.toBeInTheDocument()
  })

  it('does not render delete operation at all', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"delete","id":"abc"}',
      callId: 'call-7',
      result: "Deleted 'Task' (abc) and 0 descendant(s)",
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.queryByText('Todo Tree')).not.toBeInTheDocument()
    expect(screen.queryByText('delete')).not.toBeInTheDocument()
  })

  it('renders get operation as tree', () => {
    const block: Block = {
      type: 'tool_call',
      toolName: 'todo_tree',
      args: '{"operation":"get","id":"1"}',
      callId: 'call-8',
      result: todoTreeResult,
    }
    render(<ChatBlocks blocks={[block]} />)
    expect(screen.getByText('Todo Tree')).toBeInTheDocument()
    expect(screen.getByText('Root')).toBeInTheDocument()
  })
})
