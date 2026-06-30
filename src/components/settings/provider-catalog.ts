export interface ModelInfo {
  id: string
  label?: string
  metadata?: Record<string, string>
}

export interface ProviderCategory {
  id: string
  label: string
  defaultProviderType: string
  defaultEndpoint: string
  knownModels: ModelInfo[]
  canFetchModels: boolean
}

export const PROVIDERS: ProviderCategory[] = [
  {
    id: 'openai',
    label: 'ChatGPT',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://api.openai.com/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'gpt-4o', metadata: { max_context: '128000', vision: 'true' } },
      { id: 'gpt-4o-mini', metadata: { max_context: '128000', vision: 'true' } },
      { id: 'gpt-4-turbo', metadata: { max_context: '128000' } },
      { id: 'gpt-3.5-turbo', metadata: { max_context: '16385' } },
      { id: 'o1', metadata: { max_context: '200000' } },
      { id: 'o3-mini', metadata: { max_context: '200000', thinking_format: 'reasoning_content' } },
    ],
  },
  {
    id: 'anthropic',
    label: 'Anthropic',
    defaultProviderType: 'anthropic',
    defaultEndpoint: 'https://api.anthropic.com/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'claude-sonnet-4-20250514', label: 'Claude Sonnet 4', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-opus-4-6', label: 'Claude Opus 4.6', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-opus-4-5', label: 'Claude Opus 4.5', metadata: { max_context: '200000', thinking_format: 'thinking_block' } },
      { id: 'claude-haiku-4-5', label: 'Claude Haiku 4.5', metadata: { max_context: '200000' } },
    ],
  },
  {
    id: 'deepseek',
    label: 'DeepSeek',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://api.deepseek.com',
    canFetchModels: false,
    knownModels: [
      { id: 'deepseek-v4-flash', label: 'DeepSeek V4 Flash', metadata: { max_context: '128000', thinking_format: 'reasoning_content' } },
      { id: 'deepseek-v4-pro', label: 'DeepSeek V4 Pro', metadata: { max_context: '128000', thinking_format: 'reasoning_content' } },
      { id: 'deepseek-chat', label: 'DeepSeek Chat (legacy)', metadata: { max_context: '128000' } },
      { id: 'deepseek-reasoner', label: 'DeepSeek Reasoner (legacy)', metadata: { max_context: '128000', thinking_format: 'reasoning_content' } },
    ],
  },
  {
    id: 'ollama',
    label: 'Ollama',
    defaultProviderType: 'openai',
    defaultEndpoint: 'http://localhost:11434/v1',
    canFetchModels: true,
    knownModels: [],
  },
  {
    id: 'opencode-zen',
    label: 'OpenCode Zen',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://opencode.ai/zen/v1/chat/completions',
    canFetchModels: true,
    knownModels: [
      { id: 'gpt-5.5', metadata: {} },
      { id: 'gpt-5.5-pro', metadata: {} },
      { id: 'gpt-5.4', metadata: {} },
      { id: 'gpt-5.4-pro', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'gpt-5.4-mini', metadata: {} },
      { id: 'gpt-5.4-nano', metadata: {} },
      { id: 'gpt-5.3-codex', metadata: {} },
      { id: 'gpt-5.3-codex-spark', metadata: {} },
      { id: 'gpt-5.2', metadata: {} },
      { id: 'gpt-5.2-codex', metadata: {} },
      { id: 'gpt-5.1', metadata: {} },
      { id: 'gpt-5.1-codex', metadata: {} },
      { id: 'gpt-5.1-codex-max', metadata: {} },
      { id: 'gpt-5.1-codex-mini', metadata: {} },
      { id: 'gpt-5', metadata: {} },
      { id: 'gpt-5-codex', metadata: {} },
      { id: 'gpt-5-nano', metadata: {} },
      { id: 'deepseek-v4-flash', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'deepseek-v4-pro', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'qwen3.6-plus', metadata: {} },
      { id: 'qwen3.5-plus', metadata: {} },
      { id: 'minimax-m2.7', metadata: {} },
      { id: 'minimax-m2.5', metadata: {} },
      { id: 'glm-5.2', metadata: {} },
      { id: 'glm-5.1', metadata: {} },
      { id: 'glm-5', metadata: {} },
      { id: 'kimi-k2.6', metadata: {} },
      { id: 'kimi-k2.5', metadata: {} },
      { id: 'grok-build-0.1', metadata: {} },
    ],
  },
  {
    id: 'opencode-zen-free',
    label: 'OpenCode Zen Free',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://opencode.ai/zen/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'gpt-5-nano', metadata: {} },
      { id: 'deepseek-v4-flash-free', metadata: { thinking_format: 'reasoning_content' } },
      { id: 'big-pickle', metadata: {} },
      { id: 'mimo-v2.5-free', metadata: {} },
      { id: 'north-mini-code-free', metadata: {} },
      { id: 'nemotron-3-ultra-free', metadata: {} },
      { id: 'qwen3.6-plus-free', metadata: {} },
      { id: 'minimax-m3-free', metadata: {} },
    ],
  },
  {
    id: 'nvidia-nim',
    label: 'NVIDIA NIM',
    defaultProviderType: 'openai',
    defaultEndpoint: 'https://integrate.api.nvidia.com/v1',
    canFetchModels: true,
    knownModels: [
      { id: 'meta/llama-3.1-8b-instruct', metadata: {} },
      { id: 'meta/llama-3.1-70b-instruct', metadata: {} },
      { id: 'nvidia/llama-3.1-nemotron-70b-instruct', metadata: {} },
      { id: 'mistralai/mixtral-8x7b-instruct-v0.1', metadata: {} },
    ],
  },
  {
    id: 'custom',
    label: 'Custom',
    defaultProviderType: 'openai',
    defaultEndpoint: '',
    canFetchModels: false,
    knownModels: [],
  },
]

export function getProvider(id: string): ProviderCategory | undefined {
  return PROVIDERS.find((p) => p.id === id)
}
