import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { FileEntry, AppConfig, SessionSummary, SessionWithMessages, Session, SearchMatch, ReplaceOptions, ProviderInfo } from '@/types/ipc'

export async function listDirectory(path: string): Promise<FileEntry[]> {
  return invoke('list_directory', { path })
}

export async function readFile(path: string): Promise<string> {
  return invoke('read_file', { path })
}

export async function writeFile(path: string, content: string): Promise<void> {
  return invoke('write_file', { path, content })
}

export async function createDir(path: string): Promise<void> {
  return invoke('create_dir', { path })
}

export async function deleteItem(path: string): Promise<void> {
  return invoke('delete_item', { path })
}

export async function renameItem(oldPath: string, newPath: string): Promise<void> {
  return invoke('rename_item', { oldPath, newPath })
}

export async function gitStatus(path?: string): Promise<{ file: string; status: string }[]> {
  return invoke('git_status', { path: path ?? null })
}

export async function loadConfig(): Promise<AppConfig> {
  return invoke('load_config')
}

export async function saveConfig(config: Partial<AppConfig>): Promise<void> {
  return invoke('save_config', { config })
}

export async function listConversations(): Promise<SessionSummary[]> {
  return invoke('list_conversations')
}

export async function getConversation(id: string): Promise<SessionWithMessages> {
  return invoke('get_conversation', { id })
}

export async function createConversation(title: string): Promise<Session> {
  return invoke('create_conversation', { title })
}

export async function deleteConversation(id: string): Promise<void> {
  return invoke('delete_conversation', { id })
}

export async function sendMessage(
  sessionId: string,
  content: string,
  providerName?: string,
  model?: string,
): Promise<void> {
  return invoke('send_message', { sessionId, content, providerName, model: model ?? null })
}

export async function listProviders(): Promise<ProviderInfo[]> {
  return invoke('list_providers')
}

export async function fetchModels(
  providerType: string,
  endpoint: string,
  apiKey?: string,
): Promise<string[]> {
  return invoke('fetch_models', {
    providerType,
    endpoint,
    apiKey: apiKey ?? null,
  })
}

export async function testConnection(
  providerType: string,
  apiKey: string,
  model: string,
  endpoint?: string,
): Promise<string> {
  return invoke('test_connection', {
    providerType,
    apiKey,
    model,
    endpoint: endpoint ?? null,
  })
}

export async function checkUpdate(): Promise<{ available: boolean; version?: string; body?: string }> {
  try {
    return await invoke('check_update')
  } catch {
    return { available: false }
  }
}

export function onStreamChunk(cb: (text: string) => void) {
  return listen<string>('stream-chunk', (event) => cb(event.payload))
}

export function onStreamToolCall(cb: (toolCall: { id: string; name: string; arguments: Record<string, unknown> }) => void) {
  return listen('stream-tool-call', (event) => cb(event.payload as { id: string; name: string; arguments: Record<string, unknown> }))
}

export function onStreamDone(cb: () => void) {
  return listen('stream-done', () => cb())
}

export function onStreamError(cb: (error: string) => void) {
  return listen<string>('stream-error', (event) => cb(event.payload))
}

// ─── Terminal ──────────────────────────────────────────

export async function listTerminals(): Promise<string[]> {
  return invoke('list_terminals')
}

export async function spawnTerminal(shell?: string): Promise<string> {
  return invoke('spawn_terminal', { shell: shell ?? null })
}

export async function writeStdin(terminalId: string, data: string): Promise<void> {
  return invoke('write_stdin', { terminalId, data })
}

export async function resizePty(terminalId: string, cols: number, rows: number): Promise<void> {
  return invoke('resize_pty', { terminalId, cols, rows })
}

export async function killTerminal(terminalId: string): Promise<void> {
  return invoke('kill_terminal', { terminalId })
}

export function onTerminalOutput(cb: (payload: { terminal_id: string; data: string }) => void) {
  return listen<{ terminal_id: string; data: string }>('terminal:output', (event) => cb(event.payload))
}

export function onTerminalExit(cb: (payload: { terminal_id: string; code: number }) => void) {
  return listen<{ terminal_id: string; code: number }>('terminal:exit', (event) => cb(event.payload))
}

// ─── Search ────────────────────────────────────────────

export async function searchFiles(
  query: string,
  path: string,
  options?: {
    regex?: boolean
    caseSensitive?: boolean
    wholeWord?: boolean
    maxResults?: number
    glob?: string
    contextLines?: number
  },
): Promise<SearchMatch[]> {
  return invoke('search_files', {
    query,
    path,
    regex: options?.regex ?? false,
    caseSensitive: options?.caseSensitive ?? false,
    wholeWord: options?.wholeWord ?? false,
    maxResults: options?.maxResults ?? null,
    glob: options?.glob ?? null,
    contextLines: options?.contextLines ?? null,
  })
}

export async function replaceInFiles(options: ReplaceOptions): Promise<number> {
  return invoke('replace_in_files', {
    query: options.query,
    replacement: options.replacement,
    path: options.path,
    regex: options.regex ?? false,
    caseSensitive: options.case_sensitive ?? false,
    glob: options.glob ?? null,
  })
}

// ─── File System Events ────────────────────────────────

export function onFsChange(cb: (payload: { path: string; kind: string }) => void) {
  return listen<{ path: string; kind: string }>('fs:change', (event) => cb(event.payload))
}

// ─── Output & Errors ────────────────────────────────────

import type { OutputEntry, ErrorEntry } from '@/types/ipc'

export async function getOutput(source: string): Promise<OutputEntry[]> {
  return invoke('get_output', { source })
}

export async function getErrors(): Promise<ErrorEntry[]> {
  return invoke('get_errors')
}

export function onOutputAppend(cb: (entry: OutputEntry) => void) {
  return listen<OutputEntry>('output:append', (event) => cb(event.payload))
}

export function onErrorNew(cb: (entry: ErrorEntry) => void) {
  return listen<ErrorEntry>('error:new', (event) => cb(event.payload))
}

// ─── Tool Events ────────────────────────────────────────

export async function approveToolCall(callId: string): Promise<void> {
  return invoke('approve_tool_call', { callId })
}

export async function rejectToolCall(callId: string): Promise<void> {
  return invoke('reject_tool_call', { callId })
}

export function onStreamToolResult(cb: (result: { call_id: string; output: string; is_error: boolean }) => void) {
  return listen('stream-tool-result', (event) => cb(event.payload as { call_id: string; output: string; is_error: boolean }))
}

export function onStreamToolPending(cb: (approval: { call_id: string; tool_name: string; arguments: Record<string, unknown> }) => void) {
  return listen<string>('stream-tool-pending', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}
