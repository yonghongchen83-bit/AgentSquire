import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { normalizeMessageRole, onFsChange, renameItem } from './ipc'

const invokeMock = vi.mocked(invoke)
const listenMock = vi.mocked(listen)

beforeEach(() => {
  invokeMock.mockReset()
  listenMock.mockReset()
})

describe('normalizeMessageRole', () => {
  it('normalizes backend enum casing to frontend lowercase roles', () => {
    expect(normalizeMessageRole('User')).toBe('user')
    expect(normalizeMessageRole('Assistant')).toBe('assistant')
    expect(normalizeMessageRole('System')).toBe('system')
    expect(normalizeMessageRole('user')).toBe('user')
  })

  it('falls back safely for unknown roles', () => {
    expect(normalizeMessageRole('Unknown')).toBe('assistant')
  })
})

describe('ipc contract mapping', () => {
  it('maps renameItem args to backend command keys', async () => {
    invokeMock.mockResolvedValue(undefined)

    await renameItem('a/old.ts', 'a/new.ts')

    expect(invokeMock).toHaveBeenCalledWith('rename_item', {
      from: 'a/old.ts',
      to: 'a/new.ts',
    })
  })

  it('subscribes to file-event and forwards payload', async () => {
    const payload = { kind: 'modify', paths: ['src/App.tsx'] }
    listenMock.mockImplementation(async (_event, handler) => {
      ;(handler as (event: { payload: typeof payload }) => void)({ payload })
      return () => {}
    })

    const cb = vi.fn()
    await onFsChange(cb)

    expect(listenMock).toHaveBeenCalledWith('file-event', expect.any(Function))
    expect(cb).toHaveBeenCalledWith(payload)
  })
})
