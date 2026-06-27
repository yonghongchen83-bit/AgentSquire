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

export interface Conversation {
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

export interface SearchMatch {
  file: string
  line: number
  column: number
  content: string
  contextLines: string[]
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

// ─── Config ─────────────────────────────────────────────

export interface AppConfig {
  theme: 'light' | 'dark' | 'system'
  fontSize: number
  llmProviders: LlmProviderConfig[]
  searchExclude: string[]
  terminalShell: string
}

export interface LlmProviderConfig {
  id: string
  name: string
  apiKey: string
  model: string
  endpoint?: string
}
