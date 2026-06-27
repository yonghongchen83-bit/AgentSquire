// IPC contract — mirrors Rust serde structs
// Frontend ↔ Backend boundary

// ─── Chat ───────────────────────────────────────────────

export interface ChatRequest {
  sessionId: string
  message: string
  providerId: string
}

export interface ChatResponse {
  sessionId: string
  blocks: Block[]
}

export interface SessionSummary {
  id: string
  title: string
  messageCount: number
  lastMessageAt: string
  createdAt: string
}

export interface Message {
  id: string
  sessionId: string
  role: 'user' | 'assistant' | 'system'
  content: string
  createdAt: string
}

export interface SessionWithMessages {
  session: Session
  messages: Message[]
}

export interface Session {
  id: string
  title: string
  createdAt: string
  updatedAt: string
}

export type Block =
  | TextBlock
  | ThinkingBlock
  | ToolCallBlock
  | CodeBlock

export interface TextBlock {
  type: 'text'
  content: string
}

export interface ThinkingBlock {
  type: 'thinking'
  content: string
}

export interface ToolCallBlock {
  type: 'tool_call'
  toolName: string
  args: string
  result?: string
  callId?: string
  isPending?: boolean
  isError?: boolean
}

export interface ToolResult {
  call_id: string
  output: string
  is_error: boolean
}

export interface ToolApprovalRequest {
  call_id: string
  tool_name: string
  arguments: Record<string, unknown>
}

export interface CodeBlock {
  type: 'code'
  language: string
  content: string
}

// ─── Files ──────────────────────────────────────────────

export interface FileEntry {
  name: string
  path: string
  isDir: boolean
  size?: number
  modifiedAt?: string
}

export interface ReadFileResult {
  path: string
  content: string
}

export interface WriteFileRequest {
  path: string
  content: string
}

// ─── Search ─────────────────────────────────────────────

export interface SearchQuery {
  pattern: string
  path: string
  regex?: boolean
  caseSensitive?: boolean
  wholeWord?: boolean
  include?: string
  exclude?: string
}

// Mirrors Rust's search::grep::SearchMatch (snake_case from serde)
export interface SearchMatch {
  file: string
  line_number: number
  column: number
  content: string
  context_before: string[]
  context_after: string[]
}

export interface ReplaceOptions {
  query: string
  replacement: string
  path: string
  regex?: boolean
  case_sensitive?: boolean
  glob?: string
}

// ─── Git ────────────────────────────────────────────────

export interface GitStatusEntry {
  file: string
  status: 'modified' | 'added' | 'deleted' | 'renamed' | 'staged'
}

export interface GitDiff {
  file: string
  hunks: { oldStart: number; newStart: number; content: string }[]
}

// ─── Output & Errors ────────────────────────────────────

export interface OutputEntry {
  source: 'stdout' | 'debug' | 'notifications'
  line: string
  timestamp: string
}

export interface ErrorEntry {
  id: string
  message: string
  severity: 'error' | 'warning' | 'info'
  source?: string
  timestamp: string
  stackTrace?: string
}

// ─── Config ─────────────────────────────────────────────

export interface AppConfig {
  theme: 'light' | 'dark' | 'system'
  fontSize: number
  tabSize: number
  wordWrap: boolean
  llmProviders: LlmProviderConfig[]
  searchExclude: string[]
  terminalShell: string
  terminalFontSize: number
}

export interface LlmProviderConfig {
  id: string
  name: string
  apiKey: string
  model: string
  endpoint?: string
}
