import { describe, expect, it } from 'vitest'
import { normalizeMessageRole } from './ipc'

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
