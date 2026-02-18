use crate::{AdapterError, ExportReport, NativeSessionRef};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use stead_session_model::{
    BackendKind, EventKind, EventPayload, SessionMetadata, SessionSource, SteadEvent, SteadSession,
    build_session_uid, canonical_sort_events, schema_version,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CodexAdapter {
    pub base_dir: PathBuf,
}

impl CodexAdapter {
    pub fn from_base_dir(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<NativeSessionRef>, AdapterError> {
        let mut sessions = Vec::new();
        for path in self.session_files() {
            if let Ok(summary) = parse_summary(&path) {
                sessions.push(summary);
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn import_session(&self, session_id: &str) -> Result<SteadSession, AdapterError> {
        for path in self.session_files() {
            if let Ok(summary) = parse_summary(&path) {
                if summary.native_id == session_id {
                    return self.import_from_file(&path);
                }
            }
        }
        Err(AdapterError::SessionNotFound(session_id.to_string()))
    }

    pub fn import_from_file(&self, path: impl AsRef<Path>) -> Result<SteadSession, AdapterError> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);

        let mut original_id: Option<String> = None;
        let mut project_root = "/unknown".to_string();
        let mut first_user_text: Option<String> = None;
        let mut created: Option<DateTime<Utc>> = None;
        let mut updated: Option<DateTime<Utc>> = None;
        let mut events: Vec<SteadEvent> = Vec::new();
        let mut raw_lines: Vec<Value> = Vec::new();

        for (line_number, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let envelope: CodexEnvelope = serde_json::from_str(&line)?;
            raw_lines.push(serde_json::to_value(&envelope)?);

            let ts = parse_ts(envelope.timestamp.as_deref()).unwrap_or_else(Utc::now);
            if created.is_none() || created.is_some_and(|v| ts < v) {
                created = Some(ts);
            }
            if updated.is_none() || updated.is_some_and(|v| ts > v) {
                updated = Some(ts);
            }

            match envelope.kind.as_str() {
                "session_meta" => {
                    if let Some(payload) = envelope.payload {
                        if let Some(id) = payload.id {
                            original_id = Some(id);
                        }
                        if let Some(cwd) = payload.cwd {
                            project_root = cwd;
                        }
                    }
                }
                "response_item" => {
                    if let Some(payload) = envelope.payload {
                        let raw_payload = serde_json::to_value(&payload).unwrap_or_else(|_| json!({}));
                        let item_type = payload.item_type.as_deref().unwrap_or_default();
                        match item_type {
                            "message" => {
                                let role = payload.role.as_deref().unwrap_or_default();
                                for (text_index, text) in
                                    extract_message_texts(&payload.content).into_iter().enumerate()
                                {
                                    if role == "user" && first_user_text.is_none() {
                                        first_user_text = Some(text.clone());
                                    }
                                    let kind = if role == "assistant" {
                                        EventKind::MessageAssistant
                                    } else {
                                        EventKind::MessageUser
                                    };
                                    events.push(SteadEvent {
                                        event_uid: format!("event-{}-{}", line_number, text_index),
                                        stream_id: "main".to_string(),
                                        line_number: line_number as u64,
                                        sequence: None,
                                        timestamp: ts,
                                        kind,
                                        actor: None,
                                        payload: EventPayload::text(text),
                                        raw_vendor_payload: json!({
                                            "type": "response_item",
                                            "payload": raw_payload
                                        }),
                                        extensions: Map::new(),
                                    });
                                }
                            }
                            "function_call" => {
                                let name = payload
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string());
                                let arguments = payload
                                    .arguments
                                    .as_deref()
                                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                                    .unwrap_or_else(|| {
                                        json!({ "raw": payload.arguments.clone().unwrap_or_default() })
                                    });
                                events.push(SteadEvent {
                                    event_uid: payload
                                        .call_id
                                        .clone()
                                        .unwrap_or_else(|| format!("event-{}", line_number)),
                                    stream_id: "main".to_string(),
                                    line_number: line_number as u64,
                                    sequence: None,
                                    timestamp: ts,
                                    kind: EventKind::ToolCall,
                                    actor: None,
                                    payload: EventPayload::tool_call(name, arguments),
                                    raw_vendor_payload: json!({
                                        "type": "response_item",
                                        "payload": raw_payload
                                    }),
                                    extensions: Map::new(),
                                });
                            }
                            "function_call_output" => {
                                events.push(SteadEvent {
                                    event_uid: format!("event-{}", line_number),
                                    stream_id: "main".to_string(),
                                    line_number: line_number as u64,
                                    sequence: None,
                                    timestamp: ts,
                                    kind: EventKind::ToolResult,
                                    actor: None,
                                    payload: EventPayload::ToolResult {
                                        call_id: payload.call_id.unwrap_or_default(),
                                        ok: true,
                                        output_text: payload.output.clone(),
                                        error_text: None,
                                    },
                                    raw_vendor_payload: json!({
                                        "type": "response_item",
                                        "payload": raw_payload
                                    }),
                                    extensions: Map::new(),
                                });
                            }
                            _ => {}
                        }
                    }
                }
                "event_msg" => {
                    if let Some(payload) = envelope.payload {
                        if payload.item_type.as_deref() == Some("token_count") {
                            events.push(SteadEvent {
                                event_uid: format!("event-{}", line_number),
                                stream_id: "main".to_string(),
                                line_number: line_number as u64,
                                sequence: None,
                                timestamp: ts,
                                kind: EventKind::SystemProgress,
                                actor: None,
                                payload: EventPayload::Json {
                                    value: json!({ "token_count": payload.info }),
                                },
                                raw_vendor_payload: json!({
                                    "type": "event_msg",
                                    "payload": payload
                                }),
                                extensions: Map::new(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        canonical_sort_events(&mut events);
        let original_id = original_id.unwrap_or_else(|| {
            path.as_ref()
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        Ok(SteadSession {
            schema_version: schema_version().to_string(),
            session_uid: build_session_uid(BackendKind::Codex, &original_id),
            source: SessionSource::new(
                BackendKind::Codex,
                &original_id,
                vec![path.as_ref().display().to_string()],
            ),
            metadata: SessionMetadata::new(
                first_user_text,
                project_root,
                created.unwrap_or_else(Utc::now),
                updated.unwrap_or_else(Utc::now),
            ),
            events,
            artifacts: vec![],
            capabilities: Map::new(),
            extensions: Map::new(),
            raw_vendor_payload: json!({ "lines": raw_lines }),
        })
    }

    pub fn export_session(
        &self,
        session: &SteadSession,
        output_path: impl AsRef<Path>,
    ) -> Result<ExportReport, AdapterError> {
        let mut file = File::create(output_path.as_ref())?;
        let ts = session.metadata.created_at.to_rfc3339();
        let session_meta = json!({
            "timestamp": ts,
            "type": "session_meta",
            "payload": {
                "id": session.source.original_session_id,
                "cwd": session.metadata.project_root,
                "model_provider": "unknown"
            }
        });
        writeln!(file, "{}", serde_json::to_string(&session_meta)?)?;

        for event in &session.events {
            let line = event_to_codex_line(event);
            writeln!(file, "{}", serde_json::to_string(&line)?)?;
        }

        Ok(ExportReport {
            output_path: output_path.as_ref().to_path_buf(),
            events_exported: session.events.len(),
            warnings: vec![],
            losses: vec![],
        })
    }

    fn session_files(&self) -> Vec<PathBuf> {
        let sessions_root = self.base_dir.join("sessions");
        if !sessions_root.exists() {
            return Vec::new();
        }
        let mut files = Vec::new();
        for entry in WalkDir::new(sessions_root).into_iter().flatten() {
            if entry.path().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("jsonl"))
            {
                files.push(entry.path().to_path_buf());
            }
        }
        files
    }
}

fn event_to_codex_line(event: &SteadEvent) -> Value {
    let timestamp = event.timestamp.to_rfc3339();
    match (&event.kind, &event.payload) {
        (EventKind::MessageUser, EventPayload::Text { text }) => json!({
            "timestamp": timestamp,
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": text }]
            }
        }),
        (EventKind::MessageAssistant, EventPayload::Text { text }) => json!({
            "timestamp": timestamp,
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": text }]
            }
        }),
        (EventKind::ToolCall, EventPayload::ToolCall { tool_name, input }) => json!({
            "timestamp": timestamp,
            "type": "response_item",
            "payload": {
                "type": "function_call",
                "name": tool_name,
                "call_id": event.event_uid,
                "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string())
            }
        }),
        (
            EventKind::ToolResult,
            EventPayload::ToolResult {
                call_id,
                output_text,
                ..
            },
        ) => json!({
            "timestamp": timestamp,
            "type": "response_item",
            "payload": {
                "type": "function_call_output",
                "call_id": call_id,
                "output": output_text.clone().unwrap_or_default()
            }
        }),
        (EventKind::SystemProgress, EventPayload::Json { value }) => json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": value.get("token_count").cloned().unwrap_or(Value::Null)
            }
        }),
        _ => json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "adapter_passthrough",
                "event_kind": format!("{:?}", event.kind)
            }
        }),
    }
}

fn parse_summary(path: &Path) -> Result<NativeSessionRef, AdapterError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut id: Option<String> = None;
    let mut project_root: Option<String> = None;
    let mut updated: Option<DateTime<Utc>> = None;
    let mut title: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        let envelope: CodexEnvelope = serde_json::from_str(&line)?;
        let ts = parse_ts(envelope.timestamp.as_deref()).unwrap_or_else(Utc::now);
        if updated.is_none() || updated.is_some_and(|v| ts > v) {
            updated = Some(ts);
        }
        if envelope.kind == "session_meta" {
            if let Some(payload) = envelope.payload.as_ref() {
                if id.is_none() {
                    id = payload.id.clone();
                }
                if project_root.is_none() {
                    project_root = payload.cwd.clone();
                }
            }
        }
        if envelope.kind == "response_item" && title.is_none() {
            if let Some(payload) = envelope.payload.as_ref() {
                if payload.item_type.as_deref() == Some("message")
                    && payload.role.as_deref() == Some("user")
                {
                    title = extract_message_texts(&payload.content).into_iter().next();
                }
            }
        }
    }

    Ok(NativeSessionRef {
        native_id: id.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("unknown")
                .to_string()
        }),
        file_path: path.to_path_buf(),
        updated_at: updated.unwrap_or_else(Utc::now),
        project_root,
        title,
    })
}

fn parse_ts(raw: Option<&str>) -> Option<DateTime<Utc>> {
    raw.and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

fn extract_message_texts(content: &Option<Vec<CodexContent>>) -> Vec<String> {
    content
        .as_ref()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.text.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CodexEnvelope {
    #[serde(rename = "type")]
    kind: String,
    timestamp: Option<String>,
    payload: Option<CodexPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CodexPayload {
    id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    role: Option<String>,
    content: Option<Vec<CodexContent>>,
    name: Option<String>,
    call_id: Option<String>,
    arguments: Option<String>,
    output: Option<String>,
    info: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CodexContent {
    #[serde(rename = "type")]
    _kind: Option<String>,
    text: Option<String>,
}
