use async_trait::async_trait;
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use super::conversation_store::{
    ConversationStore, Message, MessageRole, NewMessage, NewSession, Session,
    SessionId, SessionSummary, SessionWithMessages, StoreError,
};
use crate::state::db::Database;

#[async_trait]
impl ConversationStore for Database {
    async fn create_session(&self, new: NewSession) -> Result<Session, StoreError> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let conn = self.connection();
        conn.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id.to_string(), new.title, now, now],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(Session {
            id,
            title: new.title,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let conn = self.connection();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id.to_string(), msg.session_id.to_string(), msg.role.as_str(), msg.content, now],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, msg.session_id.to_string()],
        )
        .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(Message {
            id,
            session_id: msg.session_id,
            role: msg.role,
            content: msg.content,
            created_at: Utc::now(),
        })
    }

    async fn get_session(&self, id: SessionId) -> Result<SessionWithMessages, StoreError> {
        let conn = self.connection();
        let session = conn
            .query_row(
                "SELECT id, title, created_at, updated_at FROM sessions WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    let id_str: String = row.get(0)?;
                    Ok(Session {
                        id: Uuid::parse_str(&id_str).unwrap_or_default(),
                        title: row.get(1)?,
                        created_at: row.get::<_, String>(2)?.parse().unwrap_or_default(),
                        updated_at: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                    })
                },
            )
            .map_err(|_| StoreError::NotFound(id.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, session_id, role, content, created_at FROM messages WHERE session_id = ?1 ORDER BY created_at")
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let messages: Vec<Message> = stmt
            .query_map(params![id.to_string()], |row| {
                let id_str: String = row.get(0)?;
                let session_id_str: String = row.get(1)?;
                Ok(Message {
                    id: Uuid::parse_str(&id_str).unwrap_or_default(),
                    session_id: Uuid::parse_str(&session_id_str).unwrap_or_default(),
                    role: MessageRole::from_str(&row.get::<_, String>(2)?).unwrap_or(MessageRole::User),
                    content: row.get(3)?,
                    created_at: row.get::<_, String>(4)?.parse().unwrap_or_default(),
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(SessionWithMessages { session, messages })
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, StoreError> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.title, COUNT(m.id) as msg_count, MAX(m.created_at) as last_msg, s.created_at
                 FROM sessions s
                 LEFT JOIN messages m ON m.session_id = s.id
                 GROUP BY s.id
                 ORDER BY last_msg DESC",
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let summaries: Vec<SessionSummary> = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                Ok(SessionSummary {
                    id: Uuid::parse_str(&id_str).unwrap_or_default(),
                    title: row.get(1)?,
                    message_count: row.get(2)?,
                    last_message_at: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_default(),
                    created_at: row.get::<_, String>(4)?.parse().unwrap_or_default(),
                })
            })
            .map_err(|e| StoreError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(summaries)
    }

    async fn update_session_title(&self, id: SessionId, title: String) -> Result<(), StoreError> {
        let conn = self.connection();
        let now = Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, id.to_string()],
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        if affected == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn delete_session(&self, id: SessionId) -> Result<(), StoreError> {
        let conn = self.connection();
        let affected = conn
            .execute(
                "DELETE FROM sessions WHERE id = ?1",
                params![id.to_string()],
            )
            .map_err(|e| StoreError::Database(e.to_string()))?;
        if affected == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }
}
