import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { SquireSettingsPanel } from '@/components/squire-settings-panel'

const loadConfig = vi.fn()
const saveConfig = vi.fn()

vi.mock('@/lib/ipc', () => ({
  loadConfig: (...args: unknown[]) => loadConfig(...args),
  saveConfig: (...args: unknown[]) => saveConfig(...args),
}))

describe('SquireSettingsPanel', () => {
  beforeEach(() => {
    loadConfig.mockReset()
    saveConfig.mockReset()
  })

  it('loads values from config', async () => {
    loadConfig.mockResolvedValue({
      squirePrefetch: {
        memoryTopK: 11,
        workflowTopK: 4,
        toolTopK: 5,
        skillTopK: 6,
      },
    })

    render(<SquireSettingsPanel />)

    await waitFor(() => {
      expect(screen.getByLabelText('Memory Prefetch (Top K)')).toHaveValue(11)
      expect(screen.getByLabelText('Workflow Prefetch (Top K)')).toHaveValue(4)
      expect(screen.getByLabelText('Tool Prefetch (Top K)')).toHaveValue(5)
      expect(screen.getByLabelText('Skill Prefetch (Top K)')).toHaveValue(6)
    })
  })

  it('saves clamped values', async () => {
    loadConfig.mockResolvedValue({ squirePrefetch: undefined })
    saveConfig.mockResolvedValue(undefined)

    render(<SquireSettingsPanel />)

    await waitFor(() => {
      expect(screen.getByLabelText('Memory Prefetch (Top K)')).toBeInTheDocument()
    })

    fireEvent.change(screen.getByLabelText('Memory Prefetch (Top K)'), { target: { value: '0' } })
    fireEvent.change(screen.getByLabelText('Workflow Prefetch (Top K)'), { target: { value: '120' } })
    fireEvent.change(screen.getByLabelText('Tool Prefetch (Top K)'), { target: { value: '7' } })
    fireEvent.change(screen.getByLabelText('Skill Prefetch (Top K)'), { target: { value: '8' } })

    fireEvent.click(screen.getByRole('button', { name: 'Save' }))

    await waitFor(() => {
      expect(saveConfig).toHaveBeenCalledWith({
        squirePrefetch: {
          memoryTopK: 10,
          workflowTopK: 100,
          toolTopK: 7,
          skillTopK: 8,
        },
      })
    })
  })
})
