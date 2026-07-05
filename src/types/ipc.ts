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
  contextMode: ContextMode
}

export interface Message {
  id: string
  sessionId: string
  role: 'user' | 'assistant' | 'system'
  content: string
  createdAt: string
  blocks?: Block[]
  thinkingContent?: string
}

export interface SessionWithMessages {
  session: Session
  messages: Message[]
}

export type ContextMode = 'legacy' | 'squire'

export interface Session {
  id: string
  title: string
  createdAt: string
  updatedAt: string
  contextMode: ContextMode
}

export type Block =
  | TextBlock
  | ThinkingBlock
  | ToolCallBlock
  | CodeBlock
  | SubagentBlock

export interface TextBlock {
  type: 'text'
  content: string
}

export interface ThinkingBlock {
  type: 'thinking'
  content: string
}

export interface AnalyzedPath {
  original: string
  resolved: string
  isOutsideWorkspace: boolean
}

export interface CommandAnalysis {
  command: string
  args: string[]
  paths: AnalyzedPath[]
}

export interface ToolCallBlock {
  type: 'tool_call'
  toolName: string
  args: string
  result?: string
  callId?: string
  isPending?: boolean
  isError?: boolean
  /** Command analysis for terminal tools */
  commandAnalysis?: CommandAnalysis
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
  /** Command analysis (enriched for run_terminal) */
  commandAnalysis?: CommandAnalysis
}

/** sa-5: Squire response-field AskUser loop — a paused turn awaiting a
 *  free-text answer from the user (context_squire_spec_v2.md §8.2/§9.3). */
export interface AskUserQuestion {
  question_id: string
  session_id: string
  question: string
}

export interface CodeBlock {
  type: 'code'
  language: string
  content: string
}

export interface SubagentBlock {
  type: 'subagent'
  sessionId: string
  task: string
  status: 'running' | 'completed' | 'error'
  result?: string
}

export interface SubagentInfo {
  sessionId: string
  parentCallId: string
  task: string
  status: 'running' | 'completed' | 'error'
  providerName?: string
  model?: string
}

// ─── Files ──────────────────────────────────────────────

export interface FileEntry {
  name: string
  path: string
  isDir: boolean
  isSymlink?: boolean
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
  source: 'stdout' | 'debug' | 'notifications' | 'chat'
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

// ─── Provider Info (from registry) ──────────────────────

export interface ProviderInfo {
  name: string
  provider_type: string
  models: string[]
  default_model: string
}

// ─── Config ─────────────────────────────────────────────

export interface AppConfig {
  theme: 'light' | 'dark' | 'system'
  fontSize: number
  tabSize: number
  wordWrap: boolean
  llmProviders: LlmProviderConfig[]
  mcpServers: McpServerConfig[]
  searchExclude: string[]
  terminalShell: string
  terminalFontSize: number
  verboseLogging: boolean
  leftPanelWidth?: number
  rightPanelWidth?: number
  bottomPanelHeight?: number
}

export interface LlmProviderConfig {
  providerType: string
  name: string
  apiKey: string
  model: string
  models: string[]
  endpoint?: string
  metadata?: Record<string, string>
  category?: string
}

// ─── Tools ──────────────────────────────────────────────

export interface ToolInfo {
  name: string
  description: string
  category: string
  serverName?: string
  danger: string
  enabled: boolean
}

export interface McpServerConfig {
  id: string
  name: string
  transport?: 'stdio' | 'http' | 'sse'
  command: string
  args: string[]
  url?: string
  enabled: boolean
  env?: Record<string, string>
  headers?: Record<string, string>
}
