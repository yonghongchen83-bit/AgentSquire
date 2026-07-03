use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type SessionId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub context_mode: ContextMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSession {
    pub title: String,
    #[serde(default)]
    pub context_mode: Option<ContextMode>,
}

/// Per-session context construction strategy (see ContextManagerAdapter).
/// Chosen at session creation and immutable afterward: there is no IPC
/// command or store method to change a session's mode once created — the
/// only way to use a different mode is to start a new session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContextMode {
    Legacy,
    Squire,
}

impl Default for ContextMode {
    fn default() -> Self {
        Self::Legacy
    }
}

impl ContextMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Squire => "squire",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "legacy" => Some(Self::Legacy),
            "squire" => Some(Self::Squire),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub session_id: SessionId,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub blocks_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMessage {
    pub session_id: SessionId,
    pub role: MessageRole,
    pub content: String,
    pub thinking_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub message_count: i64,
    pub last_message_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub context_mode: ContextMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWithMessages {
    pub session: Session,
    pub messages: Vec<Message>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait ConversationStore: Send + Sync {
    async fn create_session(&self, session: NewSession) -> Result<Session, StoreError>;
    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError>;
    async fn get_session(&self, id: SessionId) -> Result<SessionWithMessages, StoreError>;
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, StoreError>;
    async fn update_session_title(&self, id: SessionId, title: String) -> Result<(), StoreError>;
    async fn delete_session(&self, id: SessionId) -> Result<(), StoreError>;
    async fn truncate_messages_from(&self, session_id: SessionId, message_id: Uuid) -> Result<(), StoreError>;
    async fn set_message_blocks(&self, message_id: Uuid, blocks_json: String) -> Result<(), StoreError>;
}
