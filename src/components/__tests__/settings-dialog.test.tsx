import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { SettingsDialog } from '@/components/settings-dialog'
import { useSettingsStore } from '@/stores/settings-store'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({}),
}))

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>)
}

describe('SettingsDialog', () => {
  beforeEach(() => {
    useSettingsStore.setState({
      open: true,
      config: {
        theme: 'light',
        fontSize: 14,
        tabSize: 4,
        wordWrap: false,
        llmProviders: [
          { providerType: 'openai', name: 'My OpenAI', apiKey: 'sk-xxx', model: 'gpt-4', models: [], endpoint: '', category: 'openai' },
        ],
        searchExclude: ['node_modules', '.git'],
        terminalShell: '',
        terminalFontSize: 13,
      },
    })
  })

  it('renders with config data', () => {
    renderWithProviders(<SettingsDialog />)
    expect(screen.getByText('Settings')).toBeInTheDocument()
    expect(screen.getByText('General')).toBeInTheDocument()
    expect(screen.getByText('LLM')).toBeInTheDocument()
    expect(screen.getByText('Search')).toBeInTheDocument()
    expect(screen.getByText('Terminal')).toBeInTheDocument()
  })

  it('shows Save and Cancel buttons', () => {
    renderWithProviders(<SettingsDialog />)
    expect(screen.getByText('Save')).toBeInTheDocument()
    expect(screen.getByText('Cancel')).toBeInTheDocument()
  })

  it('shows LLM provider in LLM tab after clicking tab', async () => {
    renderWithProviders(<SettingsDialog />)
    await userEvent.click(screen.getByText('LLM'))
    expect(screen.getByDisplayValue('My OpenAI')).toBeInTheDocument()
    expect(screen.getByText('gpt-4')).toBeInTheDocument()
  })

  it('renders theme selection cards', () => {
    renderWithProviders(<SettingsDialog />)
    expect(screen.getByText('light')).toBeInTheDocument()
    expect(screen.getByText('dark')).toBeInTheDocument()
    expect(screen.getByText('system')).toBeInTheDocument()
  })

  it('does not render when closed', () => {
    useSettingsStore.setState({ open: false })
    const { container } = renderWithProviders(<SettingsDialog />)
    expect(container.innerHTML).toBe('')
  })
})
