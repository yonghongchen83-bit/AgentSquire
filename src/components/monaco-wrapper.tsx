import { useEffect, useRef, useCallback } from 'react'
import Editor, { type OnMount } from '@monaco-editor/react'
import { useEditorStore } from '@/stores/editor-store'
import { useStatusBarStore } from '@/stores/ui-store'
import { useSettingsStore } from '@/stores/settings-store'
import { readFile } from '@/lib/ipc'
import { WelcomeScreen } from '@/components/welcome-screen'
import { HtmlPreview } from '@/components/html-preview'

export function MonacoWrapper() {
  const activeTabId = useEditorStore((s) => s.activeTabId)
  const tabs = useEditorStore((s) => s.tabs)
  const gotoLine = useEditorStore((s) => s.gotoLine)
  const setGotoLine = useEditorStore((s) => s.setGotoLine)
  const setLoading = useEditorStore((s) => s.setLoading)
  const markDirty = useEditorStore((s) => s.markDirty)
  const setCursorPosition = useStatusBarStore((s) => s.setCursorPosition)
  const config = useSettingsStore((s) => s.config)
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null)
  const contentRef = useRef<string>('')

  const activeTab = tabs.find((t) => t.id === activeTabId)

  useEffect(() => {
    if (!activeTab) return
    setLoading(activeTab.id, true)
    readFile(activeTab.path)
      .then((content) => {
        contentRef.current = content
        const model = editorRef.current?.getModel()
        if (model) {
          model.setValue(content)
        }
        setLoading(activeTab.id, false)
        markDirty(activeTab.id, false)
      })
      .catch(() => {
        setLoading(activeTab.id, false)
      })
  }, [activeTab?.id])

  useEffect(() => {
    if (!gotoLine || !editorRef.current) return
    const editor = editorRef.current
    requestAnimationFrame(() => {
      editor.revealLineInCenter(gotoLine)
      editor.setPosition({ lineNumber: gotoLine, column: 1 })
      editor.focus()
    })
    setGotoLine(0)
  }, [activeTab?.id, gotoLine, setGotoLine])

  useEffect(() => {
    if (editorRef.current && config) {
      editorRef.current.updateOptions({
        fontSize: config.fontSize,
        fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
        wordWrap: config.wordWrap ? 'on' : 'off',
        tabSize: config.tabSize,
      })
    }
  }, [config?.fontSize, config?.wordWrap, config?.tabSize])

  const handleMount: OnMount = (editor) => {
    editorRef.current = editor
    editor.onDidChangeCursorPosition((e) => {
      setCursorPosition(e.position.lineNumber, e.position.column)
    })
    if (contentRef.current) {
      editor.setValue(contentRef.current)
    }
    if (config) {
      editor.updateOptions({
        fontSize: config.fontSize,
        fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
        wordWrap: config.wordWrap ? 'on' : 'off',
        tabSize: config.tabSize,
      })
    }
  }

  const setViewType = useEditorStore((s) => s.setViewType)

  const handleShowCode = useCallback(() => {
    if (activeTab) setViewType(activeTab.id, 'code')
  }, [activeTab, setViewType])

  const handleChange = (value: string | undefined) => {
    if (!activeTab || value === undefined) return
    if (value !== contentRef.current) {
      markDirty(activeTab.id, true)
    }
  }

  if (!activeTab) return <WelcomeScreen />

  if (activeTab.viewType === 'preview') {
    return <HtmlPreview path={activeTab.path} onShowCode={handleShowCode} />
  }

  return (
    <div className="relative h-full w-full">
      <Editor
        key={activeTab.id}
        language={activeTab.language}
        theme={config?.theme === 'dark' ? 'vs-dark' : 'vs'}
        value={contentRef.current}
        onChange={handleChange}
        onMount={handleMount}
        loading={
          <div className="flex items-center justify-center h-full">
            <div className="w-5 h-5 border-2 border-[#4A90D9] border-t-transparent rounded-full animate-spin" />
          </div>
        }
        options={{
          minimap: { enabled: false },
          fontSize: config?.fontSize ?? 13,
          fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
          wordWrap: config?.wordWrap ? 'on' : 'off',
          tabSize: config?.tabSize ?? 2,
          scrollBeyondLastLine: false,
          automaticLayout: true,
        }}
      />
    </div>
  )
}
