use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "0.1.0";
pub const ADAPTER_VERSION: &str = "0.1.0";

pub fn schema_version() -> &'static str {
    SCHEMA_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Codex,
    ClaudeCode,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
        }
    }
}

pub fn build_session_uid(backend: BackendKind, original_session_id: &str) -> String {
    format!("stead:{}:{}", backend.as_str(), original_session_id)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSource {
    pub backend: BackendKind,
    pub original_session_id: String,
    pub source_files: Vec<String>,
    pub imported_at: DateTime<Utc>,
    pub adapter_version: String,
}

impl SessionSource {
    pub fn new(backend: BackendKind, original_session_id: &str, source_files: Vec<String>) -> Self {
        Self {
            backend,
            original_session_id: original_session_id.to_string(),
            source_files,
            imported_at: Utc::now(),
            adapter_version: ADAPTER_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub title: Option<String>,
    pub project_root: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl SessionMetadata {
    pub fn new(
        title: Option<String>,
        project_root: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            title,
            project_root,
            created_at,
            updated_at,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventActor {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_role: Option<String>,
}

impl EventActor {
    pub fn user(role: &str) -> Self {
        Self {
            role: role.to_string(),
            agent_id: None,
            vendor_role: None,
        }
    }

    pub fn assistant(role: &str) -> Self {
        Self {
            role: role.to_string(),
            agent_id: None,
            vendor_role: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    MessageUser,
    MessageAssistant,
    ToolCall,
    ToolResult,
    SystemProgress,
    SystemNote,
    SessionMarker,
    ArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    Text {
        text: String,
    },
    ToolCall {
        tool_name: String,
        input: Value,
    },
    ToolResult {
        call_id: String,
        ok: bool,
        output_text: Option<String>,
        error_text: Option<String>,
    },
    Json {
        value: Value,
    },
}

impl EventPayload {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn tool_call(tool_name: impl Into<String>, input: Value) -> Self {
        Self::ToolCall {
            tool_name: tool_name.into(),
            input,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteadEvent {
    pub event_uid: String,
    pub stream_id: String,
    pub line_number: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    pub timestamp: DateTime<Utc>,
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<EventActor>,
    pub payload: EventPayload,
    pub raw_vendor_payload: Value,
    #[serde(default)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionArtifactRef {
    pub artifact_uid: String,
    pub kind: String,
    pub source_event_uid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionLineage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_session_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork_origin_event_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteadSession {
    pub schema_version: String,
    pub session_uid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_session_uid: Option<String>,
    pub source: SessionSource,
    pub metadata: SessionMetadata,
    pub events: Vec<SteadEvent>,
    #[serde(default)]
    pub artifacts: Vec<SessionArtifactRef>,
    #[serde(default)]
    pub capabilities: Map<String, Value>,
    #[serde(default)]
    pub extensions: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<SessionLineage>,
    pub raw_vendor_payload: Value,
}

#[derive(Debug, Error)]
pub enum SteadSessionError {
    #[error("event `{event_uid}` is missing sequence")]
    MissingSequence { event_uid: String },
    #[error(
        "event sequence is not contiguous at index {index}: expected {expected}, found {found}"
    )]
    InvalidSequence {
        index: usize,
        expected: u64,
        found: u64,
    },
}

impl SteadSession {
    pub fn validate(&self) -> Result<(), SteadSessionError> {
        for (idx, event) in self.events.iter().enumerate() {
            let Some(found) = event.sequence else {
                return Err(SteadSessionError::MissingSequence {
                    event_uid: event.event_uid.clone(),
                });
            };
            let expected = idx as u64;
            if found != expected {
                return Err(SteadSessionError::InvalidSequence {
                    index: idx,
                    expected,
                    found,
                });
            }
        }
        Ok(())
    }
}

pub fn canonical_sort_events(events: &mut [SteadEvent]) {
    events.sort_by(canonical_event_cmp);
    for (idx, event) in events.iter_mut().enumerate() {
        event.sequence = Some(idx as u64);
    }
}

fn canonical_event_cmp(a: &SteadEvent, b: &SteadEvent) -> Ordering {
    a.timestamp
        .cmp(&b.timestamp)
        .then_with(|| stream_priority(&a.stream_id).cmp(&stream_priority(&b.stream_id)))
        .then_with(|| a.line_number.cmp(&b.line_number))
        .then_with(|| a.event_uid.cmp(&b.event_uid))
}

fn stream_priority(stream_id: &str) -> u8 {
    if stream_id == "main" { 0 } else { 1 }
}
