import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { FileEntry, AppConfig, SessionSummary, SessionWithMessages, Session, ContextMode, SearchMatch, ReplaceOptions, ProviderInfo, McpServerConfig, ToolApprovalRequest, ToolInfo, AskUserQuestion } from '@/types/ipc'

type RawSessionSummary = {
  id: string
  title: string
  message_count: number
  last_message_at: string
  created_at: string
  context_mode: ContextMode
}

type RawMessage = {
  id: string
  session_id: string
  role: string
  content: string
  created_at: string
  blocks_json: string | null
  thinking_content: string | null
}

type RawSession = {
  id: string
  title: string
  created_at: string
  updated_at: string
  context_mode: ContextMode
}

type RawSessionWithMessages = {
  session: RawSession
  messages: RawMessage[]
}

function mapSessionSummary(raw: RawSessionSummary): SessionSummary {
  return {
    id: raw.id,
    title: raw.title,
    messageCount: raw.message_count,
    lastMessageAt: raw.last_message_at,
    createdAt: raw.created_at,
    contextMode: raw.context_mode,
  }
}

export function normalizeMessageRole(role: string): 'user' | 'assistant' | 'system' {
  const normalized = role.toLowerCase()
  if (normalized === 'user' || normalized === 'assistant' || normalized === 'system') {
    return normalized
  }
  return 'assistant'
}

function mapMessage(raw: RawMessage) {
  let blocks: import('@/types/ipc').Block[] | undefined
  if (raw.blocks_json) {
    try { blocks = JSON.parse(raw.blocks_json) } catch { /* ignore */ }
  }
  return {
    id: raw.id,
    sessionId: raw.session_id,
    role: normalizeMessageRole(raw.role),
    content: raw.content,
    createdAt: raw.created_at,
    blocks,
    thinkingContent: raw.thinking_content ?? undefined,
  }
}

function mapSession(raw: RawSession): Session {
  return {
    id: raw.id,
    title: raw.title,
    createdAt: raw.created_at,
    updatedAt: raw.updated_at,
    contextMode: raw.context_mode,
  }
}

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
  return invoke('rename_item', { from: oldPath, to: newPath })
}

export async function gitStatus(path?: string): Promise<{ file: string; status: string }[]> {
  return invoke('git_status', { path: path ?? null })
}

export async function loadConfig(): Promise<AppConfig> {
  return invoke('load_config')
}

export async function saveConfig(config: Partial<AppConfig>): Promise<void> {
  const current = await loadConfig()
  return invoke('save_config', { newConfig: { ...current, ...config } })
}

export async function listAvailableTools(): Promise<ToolInfo[]> {
  return invoke('list_available_tools')
}

export async function setProjectPath(path: string): Promise<void> {
  return invoke('set_project_path', { path })
}

export async function getProjectPath(): Promise<string> {
  return invoke('get_project_path')
}

export async function listConversations(): Promise<SessionSummary[]> {
  const rows = await invoke<RawSessionSummary[]>('list_conversations')
  return rows.map(mapSessionSummary)
}

export async function getConversation(id: string): Promise<SessionWithMessages> {
  const raw = await invoke<RawSessionWithMessages>('get_conversation', { id })
  return {
    session: mapSession(raw.session),
    messages: raw.messages.map(mapMessage),
  }
}

export async function createConversation(title: string, contextMode?: ContextMode): Promise<Session> {
  const raw = await invoke<RawSession>('create_conversation', { title, contextMode: contextMode ?? null })
  return mapSession(raw)
}

export async function deleteConversation(id: string): Promise<void> {
  return invoke('delete_conversation', { id })
}

export async function renameConversation(id: string, title: string): Promise<void> {
  return invoke('rename_conversation', { id, title })
}

export async function sendMessage(
  sessionId: string,
  content: string,
  providerName?: string,
  model?: string,
  thinkingLevel?: 'none' | 'low' | 'mid' | 'high',
): Promise<void> {
  return invoke('send_message', {
    sessionId,
    content,
    providerName,
    model: model ?? null,
    thinkingLevel: thinkingLevel ?? null,
  })
}

export async function abortStream(sessionId: string): Promise<void> {
  return invoke('abort_stream', { sessionId })
}

export async function abortSubagent(sessionId: string): Promise<void> {
  return invoke('abort_subagent', { sessionId })
}

export async function truncateMessagesFrom(sessionId: string, messageId: string): Promise<void> {
  return invoke('truncate_messages_from', { sessionId, messageId })
}

export async function setMessageBlocks(messageId: string, blocks: import('@/types/ipc').Block[]): Promise<void> {
  return invoke('set_message_blocks', { messageId, blocksJson: JSON.stringify(blocks) })
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

export async function testMcpConnection(server: McpServerConfig): Promise<string> {
  return invoke('test_mcp_connection', { server })
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

export function onStreamThinking(cb: (text: string) => void) {
  return listen<string>('stream-thinking', (event) => cb(event.payload))
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

export function onStreamStatus(cb: (status: string) => void) {
  return listen<string>('stream-status', (event) => cb(event.payload))
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

export function onFsChange(cb: (payload: { kind: string; paths: string[] }) => void) {
  return listen<{ kind: string; paths: string[] }>('file-event', (event) => cb(event.payload))
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

export function onStreamToolPending(cb: (approval: ToolApprovalRequest) => void) {
  return listen<string>('stream-tool-pending', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}

// ─── AskUser Loop (sa-5) ────────────────────────────────

export async function answerAskUserQuestion(questionId: string, answer: string): Promise<void> {
  return invoke('answer_ask_user_question', { questionId, answer })
}

export function onStreamAskUserPending(cb: (question: AskUserQuestion) => void) {
  return listen<string>('stream-ask-user-pending', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}

// ─── Subagent Events ────────────────────────────────────

export interface SubagentCreatedPayload {
  session_id: string
  parent_call_id: string
  task: string
}

export interface SubagentChunkPayload {
  session_id: string
  text: string
}

export interface SubagentDonePayload {
  session_id: string
  result: string
  is_error: boolean
}

export function onSubagentCreated(cb: (payload: SubagentCreatedPayload) => void) {
  return listen<string>('subagent-created', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}

export function onSubagentChunk(cb: (payload: SubagentChunkPayload) => void) {
  return listen<string>('subagent-chunk', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}

export function onSubagentDone(cb: (payload: SubagentDonePayload) => void) {
  return listen<string>('subagent-done', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}

export function onSubagentError(cb: (payload: { session_id: string; error: string }) => void) {
  return listen<string>('subagent-error', (event) => {
    try {
      const parsed = JSON.parse(event.payload)
      cb(parsed)
    } catch { /* ignore parse errors */ }
  })
}
