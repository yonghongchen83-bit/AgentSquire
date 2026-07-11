use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc};

use provider_core::ModelInstance;
use provider_registry::ProviderRegistry;
use squire_store::SquireStore;

use crate::agent::squire::ToolEndpoint;
use crate::mcp::DiscoveredTool;
use crate::state::config::SquirePrefetchConfig;
use crate::storage::conversation_store::ConversationStore;

use super::traits::EventEmitter;

/// Runtime configuration bag for the engine.
///
/// Two-tier design:
/// - **Typed fields** for well-known configuration values.
/// - **`test_config`** HashMap for test-specific flags (e.g., `log_timing`,
///   `fail_on_error`). Engine code queries these via `Self::test_flag()`.
/// - **`extra`** HashMap for general extensibility.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Enable verbose debug logging throughout the engine.
    pub verbose_logging: bool,
    /// Squire context-mode prefetch configuration.
    pub squire_prefetch: SquirePrefetchConfig,
    /// Tool names that are explicitly disabled.
    pub disabled_tools: Vec<String>,
    /// Resolved MCP server configs (only enabled ones).
    pub mcp_servers: Vec<crate::state::config::McpServerConfig>,
    /// Test-specific key/value flags (e.g., `log_timing` -> `"true"`).
    pub test_config: HashMap<String, String>,
    /// General extensibility key/value pairs.
    pub extra: HashMap<String, String>,
}

impl RuntimeConfig {
    /// Check whether a test flag is set to `"true"`.
    pub fn test_flag(&self, key: &str) -> bool {
        self.test_config.get(key).map(|v| v == "true").unwrap_or(false)
    }

    /// Get a test config value by key.
    pub fn test_value(&self, key: &str) -> Option<&str> {
        self.test_config.get(key).map(|s| s.as_str())
    }
}

impl From<crate::state::config::AppConfig> for RuntimeConfig {
    fn from(cfg: crate::state::config::AppConfig) -> Self {
        Self {
            verbose_logging: cfg.verbose_logging,
            squire_prefetch: cfg.squire_prefetch,
            disabled_tools: cfg.disabled_tools,
            mcp_servers: cfg.mcp_servers.into_iter().filter(|s| s.enabled).collect(),
            test_config: HashMap::new(),
            extra: HashMap::new(),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            verbose_logging: false,
            squire_prefetch: SquirePrefetchConfig::default(),
            disabled_tools: Vec::new(),
            mcp_servers: Vec::new(),
            test_config: HashMap::new(),
            extra: HashMap::new(),
        }
    }
}

/// All runtime dependencies an engine needs to execute a turn.
///
/// Each phase has its own **independent** [`ModelInstance`]: Phase 1 and
/// Phase 2 may use different providers, different models, different
/// endpoints, different API keys, and different per-model options
/// (thinking, temperature, max_tokens).  They are never merged and one
/// never falls back to the other — the UI and headless tests supply two
/// fully-resolved instances.
///
/// This replaces ad-hoc field-by-field extraction from Tauri's `AppState`.
/// The real app constructs `RuntimeContext` in `setup_app_impl` / command
/// handlers. Tests construct it directly with in-memory/test doubles.
pub struct RuntimeContext {
    /// The active provider registry with all configured LLM providers.
    pub provider_registry: Arc<ProviderRegistry>,
    /// Conversation store (chat history).
    pub store: Arc<dyn ConversationStore>,
    /// Squire context-mode memory store.
    pub squire_store: Arc<dyn SquireStore>,
    /// Project workspace root path.
    pub project_path: String,
    /// Runtime configuration.
    pub config: RuntimeConfig,
    /// Cache of discovered MCP tools per server ID.
    pub mcp_tools_cache: Arc<std::sync::RwLock<HashMap<String, Vec<DiscoveredTool>>>>,
    /// Hash of the last-ingested tool definitions (avoids re-ingestion).
    pub tool_registry_hash: Arc<std::sync::RwLock<u64>>,
    /// Side-channel map from tool local name to endpoint metadata.
    pub tool_endpoints: HashMap<String, ToolEndpoint>,
    /// Event emitter for streaming events to the frontend.
    pub event_emitter: Arc<dyn EventEmitter>,
    /// Cancellation flag for aborting the engine mid-execution.
    pub cancelled: Arc<AtomicBool>,
    /// Model instance for Phase 1 (response generation + bookmarks).
    pub phase1_model_instance: ModelInstance,
    /// Model instance for Phase 2 (formatter pass / token generation).
    /// Independent of Phase 1 — may be same or different provider/model/options.
    pub phase2_model_instance: ModelInstance,
    /// Tauri `AppHandle` — kept as optional legacy bridge for `SubagentTool`.
    /// TODO(ServerRefactor): remove once SubagentTool is decoupled from AppHandle.
    pub app_handle: Option<tauri::AppHandle>,
}

impl RuntimeContext {
    /// Create a new `RuntimeContext` with two independent model instances.
    pub fn new(
        provider_registry: Arc<ProviderRegistry>,
        store: Arc<dyn ConversationStore>,
        squire_store: Arc<dyn SquireStore>,
        event_emitter: Arc<dyn EventEmitter>,
        phase1_model_instance: ModelInstance,
        phase2_model_instance: ModelInstance,
    ) -> Self {
        Self {
            provider_registry,
            store,
            squire_store,
            project_path: String::new(),
            config: RuntimeConfig::default(),
            mcp_tools_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            tool_registry_hash: Arc::new(std::sync::RwLock::new(0)),
            tool_endpoints: HashMap::new(),
            event_emitter,
            cancelled: Arc::new(AtomicBool::new(false)),
            phase1_model_instance,
            phase2_model_instance,
            app_handle: None,
        }
    }
}
