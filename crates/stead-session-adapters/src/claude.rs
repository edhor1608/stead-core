use crate::{AdapterError, ExportReport, NativeSessionRef};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use stead_session_model::{
    BackendKind, EventKind, EventPayload, SessionMetadata, SessionSource, SteadEvent, SteadSession,
    build_session_uid, canonical_sort_events, schema_version,
};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ClaudeAdapter {
    pub base_dir: PathBuf,
}

impl ClaudeAdapter {
    pub fn from_base_dir(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<NativeSessionRef>, AdapterError> {
        let mut by_id: HashMap<String, NativeSessionRef> = HashMap::new();
        for file in self.main_session_files() {
            let Ok(summary) = parse_summary(&file) else {
                continue;
            };
            match by_id.get(&summary.native_id) {
                Some(existing) if existing.updated_at >= summary.updated_at => {}
                _ => {
                    by_id.insert(summary.native_id.clone(), summary);
                }
            }
        }
        let mut out: Vec<NativeSessionRef> = by_id.into_values().collect();
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(out)
    }

    pub fn import_session(&self, session_id: &str) -> Result<SteadSession, AdapterError> {
        let mut main_file: Option<(PathBuf, DateTime<Utc>)> = None;
        for file in self.main_session_files() {
            let Ok(summary) = parse_summary(&file) else {
                continue;
            };
            if summary.native_id != session_id {
                continue;
            }
            match &main_file {
                Some((_, best_updated_at)) if *best_updated_at >= summary.updated_at => {}
                _ => {
                    main_file = Some((file, summary.updated_at));
                }
            }
        }

        let Some((main_file, _)) = main_file else {
            return Err(AdapterError::SessionNotFound(session_id.to_string()));
        };

        let mut session = self.import_from_file(&main_file, "main")?;
        let mut source_files = vec![main_file.display().to_string()];
        if let Some(parent) = main_file.parent() {
            let sub_dir = parent.join("subagents");
            if sub_dir.exists() {
                for entry in WalkDir::new(sub_dir).into_iter().flatten() {
                    let path = entry.path();
                    if !path.is_file() || path.extension().is_none_or(|v| v != "jsonl") {
                        continue;
                    }
                    let stream_id = format!(
                        "subagent:{}",
                        path.file_stem()
                            .and_then(|v| v.to_str())
                            .unwrap_or("unknown")
                    );
                    let sub = self.import_from_file(path, &stream_id)?;
                    if sub.source.original_session_id == session_id {
                        source_files.push(path.display().to_string());
                        session.events.extend(sub.events);
                    }
                }
            }
        }
        canonical_sort_events(&mut session.events);
        session.source.source_files = source_files;
        Ok(session)
    }

    pub fn import_from_file(
        &self,
        path: impl AsRef<Path>,
        stream_id: &str,
    ) -> Result<SteadSession, AdapterError> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);
        let mut session_id: Option<String> = None;
        let mut project_root = "/unknown".to_string();
        let mut created: Option<DateTime<Utc>> = None;
        let mut updated: Option<DateTime<Utc>> = None;
        let mut title: Option<String> = None;
        let mut raw_lines: Vec<Value> = Vec::new();
        let mut events: Vec<SteadEvent> = Vec::new();

        for (line_number, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: ClaudeEntry = serde_json::from_str(&line)?;
            raw_lines.push(serde_json::from_str(&line)?);

            if session_id.is_none() {
                session_id = entry.session_id.clone();
            }
            if entry.cwd.is_some() {
                project_root = entry.cwd.clone().unwrap_or(project_root);
            }

            if let Some(ts) = parse_ts(entry.timestamp.as_deref()) {
                if created.is_none() || created.is_some_and(|v| ts < v) {
                    created = Some(ts);
                }
                if updated.is_none() || updated.is_some_and(|v| ts > v) {
                    updated = Some(ts);
                }
            }

            let ts = parse_ts(entry.timestamp.as_deref()).unwrap_or_else(Utc::now);

            match entry.entry_type.as_deref() {
                Some("user") | Some("assistant") => {
                    if let Some(message) = entry.message {
                        let event_uuid = entry.uuid.clone().unwrap_or_else(|| "ev".to_string());
                        match message.content {
                            Content::Text(text) => {
                                if message.role == "user" && title.is_none() {
                                    title = Some(text.clone());
                                }
                                let kind = if message.role == "assistant" {
                                    EventKind::MessageAssistant
                                } else {
                                    EventKind::MessageUser
                                };
                                events.push(SteadEvent {
                                    event_uid: format!("{}-{}", event_uuid, line_number),
                                    stream_id: stream_id.to_string(),
                                    line_number: line_number as u64,
                                    sequence: None,
                                    timestamp: ts,
                                    kind,
                                    actor: None,
                                    payload: EventPayload::text(text),
                                    raw_vendor_payload: raw_lines
                                        .last()
                                        .cloned()
                                        .unwrap_or_else(|| json!({})),
                                    extensions: Map::new(),
                                });
                            }
                            Content::Items(items) => {
                                for (item_index, item) in items.into_iter().enumerate() {
                                    let item_discriminator = item
                                        .id
                                        .clone()
                                        .or(item.tool_use_id.clone())
                                        .unwrap_or_else(|| format!("item-{}", item_index));
                                    let event_uid = format!(
                                        "{}-{}-{}",
                                        event_uuid, line_number, item_discriminator
                                    );
                                    let call_id = item
                                        .id
                                        .clone()
                                        .or(item.tool_use_id.clone())
                                        .unwrap_or_else(|| event_uid.clone());
                                    match item.item_type.as_deref() {
                                        Some("text") => {
                                            if let Some(text) = item.text {
                                                if message.role == "user" && title.is_none() {
                                                    title = Some(text.clone());
                                                }
                                                let kind = if message.role == "assistant" {
                                                    EventKind::MessageAssistant
                                                } else {
                                                    EventKind::MessageUser
                                                };
                                                events.push(SteadEvent {
                                                    event_uid: event_uid.clone(),
                                                    stream_id: stream_id.to_string(),
                                                    line_number: line_number as u64,
                                                    sequence: None,
                                                    timestamp: ts,
                                                    kind,
                                                    actor: None,
                                                    payload: EventPayload::text(text),
                                                    raw_vendor_payload: raw_lines
                                                        .last()
                                                        .cloned()
                                                        .unwrap_or_else(|| json!({})),
                                                    extensions: Map::new(),
                                                });
                                            }
                                        }
                                        Some("tool_use") => {
                                            events.push(SteadEvent {
                                                event_uid: call_id.clone(),
                                                stream_id: stream_id.to_string(),
                                                line_number: line_number as u64,
                                                sequence: None,
                                                timestamp: ts,
                                                kind: EventKind::ToolCall,
                                                actor: None,
                                                payload: EventPayload::tool_call(
                                                    item.name
                                                        .unwrap_or_else(|| "unknown".to_string()),
                                                    item.input.unwrap_or_else(|| json!({})),
                                                ),
                                                raw_vendor_payload: raw_lines
                                                    .last()
                                                    .cloned()
                                                    .unwrap_or_else(|| json!({})),
                                                extensions: Map::new(),
                                            });
                                        }
                                        Some("tool_result") => {
                                            events.push(SteadEvent {
                                                event_uid: format!("{}-result", event_uid),
                                                stream_id: stream_id.to_string(),
                                                line_number: line_number as u64,
                                                sequence: None,
                                                timestamp: ts,
                                                kind: EventKind::ToolResult,
                                                actor: None,
                                                payload: EventPayload::ToolResult {
                                                    call_id,
                                                    ok: !item.is_error.unwrap_or(false),
                                                    output_text: value_to_text(item.content),
                                                    error_text: None,
                                                },
                                                raw_vendor_payload: raw_lines
                                                    .last()
                                                    .cloned()
                                                    .unwrap_or_else(|| json!({})),
                                                extensions: Map::new(),
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Content::Raw(value) => {
                                let text = value.to_string();
                                if message.role == "user" && title.is_none() {
                                    title = Some(text.clone());
                                }
                                let kind = if message.role == "assistant" {
                                    EventKind::MessageAssistant
                                } else {
                                    EventKind::MessageUser
                                };
                                events.push(SteadEvent {
                                    event_uid: format!(
                                        "{}-{}",
                                        entry.uuid.clone().unwrap_or_else(|| "ev".to_string()),
                                        line_number
                                    ),
                                    stream_id: stream_id.to_string(),
                                    line_number: line_number as u64,
                                    sequence: None,
                                    timestamp: ts,
                                    kind,
                                    actor: None,
                                    payload: EventPayload::text(text),
                                    raw_vendor_payload: raw_lines
                                        .last()
                                        .cloned()
                                        .unwrap_or_else(|| json!({})),
                                    extensions: Map::new(),
                                });
                            }
                        }
                    }
                }
                Some("progress") => {
                    let progress_uid = entry
                        .uuid
                        .clone()
                        .unwrap_or_else(|| format!("progress-{}-{}", stream_id, line_number));
                    events.push(SteadEvent {
                        event_uid: progress_uid,
                        stream_id: stream_id.to_string(),
                        line_number: line_number as u64,
                        sequence: None,
                        timestamp: ts,
                        kind: EventKind::SystemProgress,
                        actor: None,
                        payload: EventPayload::Json {
                            value: entry.data.unwrap_or_else(|| json!({})),
                        },
                        raw_vendor_payload: raw_lines.last().cloned().unwrap_or_else(|| json!({})),
                        extensions: Map::new(),
                    });
                }
                _ => {}
            }
        }

        canonical_sort_events(&mut events);
        let session_id = session_id.unwrap_or_else(|| {
            path.as_ref()
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        Ok(SteadSession {
            schema_version: schema_version().to_string(),
            session_uid: build_session_uid(BackendKind::ClaudeCode, &session_id),
            source: SessionSource::new(
                BackendKind::ClaudeCode,
                &session_id,
                vec![path.as_ref().display().to_string()],
            ),
            metadata: SessionMetadata::new(
                title,
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

        for event in &session.events {
            let line = merge_with_raw_unknowns(
                event_to_claude_line(
                    event,
                    &session.source.original_session_id,
                    &session.metadata.project_root,
                ),
                Some(&event.raw_vendor_payload),
            );
            writeln!(file, "{}", serde_json::to_string(&line)?)?;
        }

        Ok(ExportReport {
            output_path: output_path.as_ref().to_path_buf(),
            events_exported: session.events.len(),
            warnings: vec![],
            losses: vec![],
        })
    }

    fn main_session_files(&self) -> Vec<PathBuf> {
        let root = if self
            .base_dir
            .file_name()
            .is_some_and(|v| v.to_string_lossy().eq_ignore_ascii_case("projects"))
        {
            self.base_dir.clone()
        } else {
            self.base_dir.join("projects")
        };
        if !root.exists() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for entry in WalkDir::new(root).into_iter().flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            if path.components().any(|c| c.as_os_str() == "subagents") {
                continue;
            }
            out.push(path.to_path_buf());
        }
        out
    }
}

fn event_to_claude_line(event: &SteadEvent, session_id: &str, cwd: &str) -> Value {
    let timestamp = event.timestamp.to_rfc3339();
    match (&event.kind, &event.payload) {
        (EventKind::MessageUser, EventPayload::Text { text }) => json!({
            "type": "user",
            "timestamp": timestamp,
            "sessionId": session_id,
            "cwd": cwd,
            "uuid": event.event_uid,
            "message": { "role": "user", "content": text }
        }),
        (EventKind::MessageAssistant, EventPayload::Text { text }) => json!({
            "type": "assistant",
            "timestamp": timestamp,
            "sessionId": session_id,
            "cwd": cwd,
            "uuid": event.event_uid,
            "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] }
        }),
        (EventKind::ToolCall, EventPayload::ToolCall { tool_name, input }) => json!({
            "type": "assistant",
            "timestamp": timestamp,
            "sessionId": session_id,
            "cwd": cwd,
            "uuid": event.event_uid,
            "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": event.event_uid, "name": tool_name, "input": input }] }
        }),
        (
            EventKind::ToolResult,
            EventPayload::ToolResult {
                call_id,
                output_text,
                ok,
                ..
            },
        ) => json!({
            "type": "user",
            "timestamp": timestamp,
            "sessionId": session_id,
            "cwd": cwd,
            "uuid": event.event_uid,
            "message": {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": call_id,
                    "content": output_text.clone().unwrap_or_default(),
                    "is_error": !ok
                }]
            }
        }),
        (EventKind::SystemProgress, EventPayload::Json { value }) => json!({
            "type": "progress",
            "timestamp": timestamp,
            "sessionId": session_id,
            "cwd": cwd,
            "uuid": event.event_uid,
            "data": value
        }),
        _ => json!({
            "type": "system",
            "timestamp": timestamp,
            "sessionId": session_id,
            "cwd": cwd,
            "uuid": event.event_uid
        }),
    }
}

fn merge_with_raw_unknowns(generated: Value, raw: Option<&Value>) -> Value {
    let Some(raw) = raw else {
        return generated;
    };
    let (Some(raw_type), Some(generated_type)) = (
        raw.get("type").and_then(|v| v.as_str()),
        generated.get("type").and_then(|v| v.as_str()),
    ) else {
        return generated;
    };
    if raw_type != generated_type {
        return generated;
    }
    merge_value_objects(raw.clone(), generated)
}

fn merge_value_objects(raw: Value, generated: Value) -> Value {
    match (raw, generated) {
        (Value::Object(mut raw_map), Value::Object(generated_map)) => {
            for (key, generated_value) in generated_map {
                let merged = match raw_map.remove(&key) {
                    Some(raw_value) => merge_value_objects(raw_value, generated_value),
                    None => generated_value,
                };
                raw_map.insert(key, merged);
            }
            Value::Object(raw_map)
        }
        (Value::Array(raw_values), Value::Array(generated_values)) => Value::Array(
            generated_values
                .into_iter()
                .enumerate()
                .map(|(index, generated_value)| match raw_values.get(index) {
                    Some(raw_value) => merge_value_objects(raw_value.clone(), generated_value),
                    None => generated_value,
                })
                .collect(),
        ),
        (_, generated_other) => generated_other,
    }
}

fn parse_summary(path: &Path) -> Result<NativeSessionRef, AdapterError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut session_id: Option<String> = None;
    let mut updated: Option<DateTime<Utc>> = None;
    let mut project_root: Option<String> = None;
    let mut title: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        let entry: ClaudeEntry = serde_json::from_str(&line)?;
        if session_id.is_none() {
            session_id = entry.session_id.clone();
        }
        if project_root.is_none() {
            project_root = entry.cwd.clone();
        }
        let ts = parse_ts(entry.timestamp.as_deref()).unwrap_or_else(Utc::now);
        if updated.is_none() || updated.is_some_and(|v| ts > v) {
            updated = Some(ts);
        }
        if title.is_none()
            && entry.entry_type.as_deref() == Some("user")
            && let Some(message) = entry.message
        {
            match message.content {
                Content::Text(text) => title = Some(text),
                Content::Items(items) => {
                    title = items.into_iter().find_map(|item| item.text);
                }
                Content::Raw(value) => {
                    let _ = value;
                }
            }
        }
    }

    Ok(NativeSessionRef {
        native_id: session_id.unwrap_or_else(|| {
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

fn value_to_text(value: Option<Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    timestamp: Option<String>,
    session_id: Option<String>,
    cwd: Option<String>,
    uuid: Option<String>,
    message: Option<ClaudeMessage>,
    data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: Content,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Items(Vec<ContentItem>),
    Raw(Value),
}

#[derive(Debug, Clone, Deserialize)]
struct ContentItem {
    #[serde(rename = "type")]
    item_type: Option<String>,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<Value>,
    tool_use_id: Option<String>,
    content: Option<Value>,
    is_error: Option<bool>,
}
