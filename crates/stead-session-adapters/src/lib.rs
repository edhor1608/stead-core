pub mod codex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeSessionRef {
    pub native_id: String,
    pub file_path: PathBuf,
    pub updated_at: DateTime<Utc>,
    pub project_root: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportReport {
    pub output_path: PathBuf,
    pub events_exported: usize,
    pub warnings: Vec<String>,
    pub losses: Vec<String>,
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("invalid format: {0}")]
    InvalidFormat(String),
}
