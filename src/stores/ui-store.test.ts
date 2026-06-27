import { describe, it, expect, beforeEach } from 'vitest'
import { useLayoutStore, useStatusBarStore } from './ui-store'

describe('LayoutStore', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      leftPanelVisible: true,
      leftPanelActiveView: 'explorer',
      leftPanelWidth: 280,
      rightPanelVisible: false,
      rightPanelWidth: 380,
      bottomPanelVisible: false,
      bottomPanelHeight: 200,
      bottomPanelActiveTab: 'terminal',
      uiFontZoom: 100,
      projectPath: '',
    })
  })

  it('toggles left panel', () => {
    expect(useLayoutStore.getState().leftPanelVisible).toBe(true)
    useLayoutStore.getState().toggleLeftPanel()
    expect(useLayoutStore.getState().leftPanelVisible).toBe(false)
    useLayoutStore.getState().toggleLeftPanel()
    expect(useLayoutStore.getState().leftPanelVisible).toBe(true)
  })

  it('sets left panel view and makes it visible', () => {
    useLayoutStore.setState({ leftPanelVisible: false })
    useLayoutStore.getState().setLeftPanelView('search')
    expect(useLayoutStore.getState().leftPanelActiveView).toBe('search')
    expect(useLayoutStore.getState().leftPanelVisible).toBe(true)
  })

  it('toggles right panel', () => {
    useLayoutStore.getState().toggleRightPanel()
    expect(useLayoutStore.getState().rightPanelVisible).toBe(true)
    useLayoutStore.getState().toggleRightPanel()
    expect(useLayoutStore.getState().rightPanelVisible).toBe(false)
  })

  it('toggles bottom panel', () => {
    useLayoutStore.getState().toggleBottomPanel()
    expect(useLayoutStore.getState().bottomPanelVisible).toBe(true)
    useLayoutStore.getState().toggleBottomPanel()
    expect(useLayoutStore.getState().bottomPanelVisible).toBe(false)
  })

  it('sets bottom panel tab', () => {
    useLayoutStore.getState().setBottomPanelTab('output')
    expect(useLayoutStore.getState().bottomPanelActiveTab).toBe('output')
    useLayoutStore.getState().setBottomPanelTab('errors')
    expect(useLayoutStore.getState().bottomPanelActiveTab).toBe('errors')
  })

  it('adjusts font zoom within bounds', () => {
    useLayoutStore.getState().setUiFontZoom(110)
    expect(useLayoutStore.getState().uiFontZoom).toBe(110)
    useLayoutStore.getState().setUiFontZoom(70)
    expect(useLayoutStore.getState().uiFontZoom).toBe(75)
    useLayoutStore.getState().setUiFontZoom(160)
    expect(useLayoutStore.getState().uiFontZoom).toBe(150)
  })
})

describe('StatusBarStore', () => {
  beforeEach(() => {
    useStatusBarStore.setState({
      llmConnected: false,
      llmProvider: '',
      notificationCount: 0,
      cursorLine: 1,
      cursorCol: 1,
    })
  })

  it('sets LLM connection status', () => {
    useStatusBarStore.getState().setLlmConnected(true, 'OpenAI')
    expect(useStatusBarStore.getState().llmConnected).toBe(true)
    expect(useStatusBarStore.getState().llmProvider).toBe('OpenAI')
  })

  it('sets cursor position', () => {
    useStatusBarStore.getState().setCursorPosition(42, 10)
    expect(useStatusBarStore.getState().cursorLine).toBe(42)
    expect(useStatusBarStore.getState().cursorCol).toBe(10)
  })

  it('sets notification count', () => {
    useStatusBarStore.getState().setNotificationCount(3)
    expect(useStatusBarStore.getState().notificationCount).toBe(3)
  })
})
