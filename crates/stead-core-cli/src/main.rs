use anyhow::{Context, Result, anyhow, bail};
use chrono::{Datelike, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::Command;
use twox_hash::XxHash64;
use uuid::Uuid;

use stead_session_adapters::NativeSessionRef;
use stead_session_adapters::claude::ClaudeAdapter;
use stead_session_adapters::codex::CodexAdapter;
use stead_session_model::{BackendKind, SteadSession};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Backend {
    Codex,
    Claude,
}

#[derive(Debug, Parser)]
#[command(name = "stead-core")]
#[command(about = "Stead core session standard CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Sessions {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Import {
        #[arg(long = "from", value_enum)]
        from: Backend,
        #[arg(long)]
        base_dir: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long)]
        out: PathBuf,
    },
    Export {
        #[arg(long = "to", value_enum)]
        to: Backend,
        #[arg(long)]
        base_dir: PathBuf,
        #[arg(long = "in", alias = "input")]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Convert {
        #[arg(long = "from", value_enum)]
        from: Backend,
        #[arg(long = "to", value_enum)]
        to: Backend,
        #[arg(long)]
        source_base: PathBuf,
        #[arg(long)]
        target_base: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long)]
        out: PathBuf,
    },
    Sync {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        codex_base: PathBuf,
        #[arg(long)]
        claude_base: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Materialize {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long = "to", value_enum)]
        to: Backend,
        #[arg(long)]
        base_dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Resume {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long, value_enum)]
        backend: Backend,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        base_dir: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Handoff {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long = "to", value_enum)]
        to: Backend,
        #[arg(long = "resume")]
        resume_prompt: String,
        #[arg(long)]
        base_dir: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommands {
    List {
        #[arg(long, value_enum)]
        backend: Backend,
        #[arg(long)]
        base_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone)]
struct StoredCanonical {
    session: SteadSession,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Sessions { command } => match command {
            SessionCommands::List {
                backend,
                base_dir,
                json,
            } => run_list(backend, base_dir, json),
        },
        Commands::Import {
            from,
            base_dir,
            session,
            out,
        } => run_import(from, base_dir, &session, out),
        Commands::Export {
            to,
            base_dir,
            input,
            out,
        } => run_export(to, base_dir, input, out),
        Commands::Convert {
            from,
            to,
            source_base,
            target_base,
            session,
            out,
        } => run_convert(from, to, source_base, target_base, &session, out),
        Commands::Sync {
            repo,
            codex_base,
            claude_base,
            json,
        } => run_sync(repo, codex_base, claude_base, json),
        Commands::Materialize {
            repo,
            session,
            to,
            base_dir,
            out,
            json,
        } => run_materialize(repo, &session, to, base_dir, out, json),
        Commands::Resume {
            repo,
            session,
            backend,
            prompt,
            base_dir,
            out,
            json,
        } => run_resume(repo, &session, backend, &prompt, base_dir, out, json),
        Commands::Handoff {
            repo,
            session,
            to,
            resume_prompt,
            base_dir,
            out,
            json,
        } => run_handoff(repo, &session, to, &resume_prompt, base_dir, out, json),
    }
}

fn run_list(backend: Backend, base_dir: PathBuf, json: bool) -> Result<()> {
    let sessions = match backend {
        Backend::Codex => CodexAdapter::from_base_dir(base_dir).list_sessions()?,
        Backend::Claude => ClaudeAdapter::from_base_dir(base_dir).list_sessions()?,
    };
    if json {
        let serialized = serde_json::to_string(&sessions).with_context(|| {
            format!(
                "failed to serialize sessions to JSON for backend {:?} ({} sessions)",
                backend,
                sessions.len()
            )
        })?;
        println!("{serialized}");
    } else {
        for session in sessions {
            println!("{} {}", session.native_id, session.file_path.display());
        }
    }
    Ok(())
}

fn run_import(from: Backend, base_dir: PathBuf, session: &str, out: PathBuf) -> Result<()> {
    let imported = match from {
        Backend::Codex => CodexAdapter::from_base_dir(base_dir).import_session(session)?,
        Backend::Claude => ClaudeAdapter::from_base_dir(base_dir).import_session(session)?,
    };
    let serialized =
        serde_json::to_string_pretty(&imported).context("failed to serialize canonical session")?;
    let parent = out
        .parent()
        .with_context(|| format!("invalid output path: {}", out.display()))?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }
    std::fs::write(&out, serialized)
        .with_context(|| format!("failed to write canonical session to {}", out.display()))?;
    Ok(())
}

fn run_export(to: Backend, base_dir: PathBuf, input: PathBuf, out: PathBuf) -> Result<()> {
    let raw = std::fs::read_to_string(&input)
        .with_context(|| format!("failed to read canonical input {}", input.display()))?;
    let session: SteadSession = serde_json::from_str(&raw)
        .with_context(|| format!("invalid canonical JSON in {}", input.display()))?;
    let parent = out
        .parent()
        .with_context(|| format!("invalid output path: {}", out.display()))?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }
    match to {
        Backend::Codex => {
            CodexAdapter::from_base_dir(base_dir).export_session(&session, &out)?;
        }
        Backend::Claude => {
            ClaudeAdapter::from_base_dir(base_dir).export_session(&session, &out)?;
        }
    }
    Ok(())
}

fn run_convert(
    from: Backend,
    to: Backend,
    source_base: PathBuf,
    target_base: PathBuf,
    session: &str,
    out: PathBuf,
) -> Result<()> {
    let imported = match from {
        Backend::Codex => CodexAdapter::from_base_dir(source_base).import_session(session)?,
        Backend::Claude => ClaudeAdapter::from_base_dir(source_base).import_session(session)?,
    };
    let parent = out
        .parent()
        .with_context(|| format!("invalid output path: {}", out.display()))?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }
    match to {
        Backend::Codex => {
            CodexAdapter::from_base_dir(target_base).export_session(&imported, &out)?;
        }
        Backend::Claude => {
            ClaudeAdapter::from_base_dir(target_base).export_session(&imported, &out)?;
        }
    }
    Ok(())
}

fn run_sync(
    repo: PathBuf,
    codex_base: PathBuf,
    claude_base: PathBuf,
    json_out: bool,
) -> Result<()> {
    std::fs::create_dir_all(canonical_store_dir(&repo))?;
    let mut stored = load_all_canonical_sessions(&repo)?;
    let mut imported = Vec::new();

    let codex = CodexAdapter::from_base_dir(codex_base);
    let codex_sessions = codex.list_sessions()?;
    for native in scope_sessions_to_repo(&repo, codex_sessions) {
        let session = codex.import_from_file(&native.file_path)?;
        let (canonical_uid, stored_path) = upsert_synced_session(
            &repo,
            &mut stored,
            session,
            Backend::Codex,
            &native.native_id,
            &native.file_path,
        )?;
        imported.push(json!({
            "backend": "codex",
            "native_id": native.native_id,
            "session_uid": canonical_uid,
            "stored_at": stored_path
        }));
    }

    let claude = ClaudeAdapter::from_base_dir(claude_base);
    let claude_sessions = claude.list_sessions()?;
    for native in scope_sessions_to_repo(&repo, claude_sessions) {
        let session = claude.import_session(&native.native_id)?;
        let (canonical_uid, stored_path) = upsert_synced_session(
            &repo,
            &mut stored,
            session,
            Backend::Claude,
            &native.native_id,
            &native.file_path,
        )?;
        imported.push(json!({
            "backend": "claude",
            "native_id": native.native_id,
            "session_uid": canonical_uid,
            "stored_at": stored_path
        }));
    }

    if json_out {
        println!("{}", serde_json::to_string(&imported)?);
    } else {
        println!(
            "synced {} sessions into {}",
            imported.len(),
            canonical_store_dir(&repo).display()
        );
    }
    Ok(())
}

fn run_materialize(
    repo: PathBuf,
    session_uid: &str,
    to: Backend,
    base_dir: PathBuf,
    out: Option<PathBuf>,
    json_out: bool,
) -> Result<()> {
    let mut session = load_canonical_session(&repo, session_uid)?;
    ensure_shared_session_uid(&mut session);
    let native_id = choose_native_id(&session, to);
    let output_path =
        out.unwrap_or_else(|| default_materialized_path(&base_dir, &repo, to, &native_id));
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if matches!(to, Backend::Codex) {
        prune_codex_rollouts_for_native_id(&base_dir, &native_id, &output_path)?;
    }

    let mut export_session = session.clone();
    export_session.source.original_session_id = native_id.clone();

    let report =
        match to {
            Backend::Codex => CodexAdapter::from_base_dir(base_dir)
                .export_session(&export_session, &output_path)?,
            Backend::Claude => ClaudeAdapter::from_base_dir(base_dir)
                .export_session(&export_session, &output_path)?,
        };

    set_native_ref(&mut session, to, &native_id, &output_path);
    ensure_shared_session_uid(&mut session);
    store_canonical_session(&repo, &session)?;

    if json_out {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "session_uid": session_uid,
                "backend": backend_key(to),
                "native_id": native_id,
                "output_path": output_path,
                "events_exported": report.events_exported
            }))?
        );
    } else {
        println!("materialized {} -> {}", session_uid, output_path.display());
    }
    Ok(())
}

fn run_resume(
    repo: PathBuf,
    session_uid: &str,
    backend: Backend,
    prompt: &str,
    base_dir: Option<PathBuf>,
    out: Option<PathBuf>,
    json_out: bool,
) -> Result<()> {
    let mut session = load_canonical_session(&repo, session_uid)?;
    let mut changed = ensure_shared_session_uid(&mut session);

    let (native_id, native_path) = if let Some(found) = get_native_ref(&session, backend) {
        found
    } else {
        let Some(base_dir) = base_dir else {
            bail!(
                "missing native projection for backend `{}`; provide --base-dir to materialize",
                backend_key(backend)
            );
        };
        let native_id = choose_native_id(&session, backend);
        let output_path =
            out.unwrap_or_else(|| default_materialized_path(&base_dir, &repo, backend, &native_id));
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if matches!(backend, Backend::Codex) {
            prune_codex_rollouts_for_native_id(&base_dir, &native_id, &output_path)?;
        }

        let mut export_session = session.clone();
        export_session.source.original_session_id = native_id.clone();
        match backend {
            Backend::Codex => {
                CodexAdapter::from_base_dir(base_dir)
                    .export_session(&export_session, &output_path)?;
            }
            Backend::Claude => {
                ClaudeAdapter::from_base_dir(base_dir)
                    .export_session(&export_session, &output_path)?;
            }
        }
        set_native_ref(&mut session, backend, &native_id, &output_path);
        changed = true;
        (native_id, output_path)
    };

    let status = if let Ok(runner) = std::env::var("STEAD_CORE_RUNNER") {
        Command::new(runner)
            .args([backend_key(backend), "--resume", &native_id, prompt])
            .status()?
    } else {
        let bin = match backend {
            Backend::Codex => {
                std::env::var("STEAD_CORE_CODEX_BIN").unwrap_or_else(|_| "codex".to_string())
            }
            Backend::Claude => {
                std::env::var("STEAD_CORE_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
            }
        };
        let mut command = Command::new(bin);
        command.current_dir(&repo);
        match backend {
            Backend::Codex => {
                command.args(["exec", "resume", &native_id, prompt]);
            }
            Backend::Claude => {
                command.args(["-p", "-r", &native_id, prompt]);
            }
        }
        command.status()?
    };

    if !status.success() {
        bail!("resume command failed for backend {}", backend_key(backend));
    }

    if json_out {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "session_uid": session_uid,
                "backend": backend_key(backend),
                "native_id": native_id,
                "native_path": native_path,
                "status": "ok"
            }))?
        );
    } else {
        println!("resumed {} on {}", session_uid, backend_key(backend));
    }

    if changed {
        store_canonical_session(&repo, &session)?;
    }

    Ok(())
}

fn run_handoff(
    repo: PathBuf,
    session_uid: &str,
    to: Backend,
    resume_prompt: &str,
    base_dir: Option<PathBuf>,
    out: Option<PathBuf>,
    json_out: bool,
) -> Result<()> {
    run_resume(
        repo,
        session_uid,
        to,
        resume_prompt,
        base_dir,
        out,
        json_out,
    )
}

fn upsert_synced_session(
    repo: &Path,
    stored: &mut Vec<StoredCanonical>,
    mut imported: SteadSession,
    backend: Backend,
    native_id: &str,
    native_path: &Path,
) -> Result<(String, PathBuf)> {
    ensure_shared_session_uid(&mut imported);
    set_native_ref(&mut imported, backend, native_id, native_path);

    let target_index = stored.iter().position(|existing| {
        existing
            .session
            .extensions
            .get("native_refs")
            .and_then(|v| v.as_object())
            .and_then(|refs| refs.get(backend_key(backend)))
            .and_then(|v| v.as_object())
            .and_then(|obj| obj.get("session_id"))
            .and_then(|v| v.as_str())
            .is_some_and(|id| id == native_id)
    });

    let target_index = target_index.or_else(|| {
        stored
            .iter()
            .position(|existing| existing.session.session_uid == imported.session_uid)
    });

    let target_index = target_index.or_else(|| {
        let imported_shared = imported
            .shared_session_uid
            .clone()
            .unwrap_or_else(|| imported.session_uid.clone());
        stored.iter().position(|existing| {
            existing
                .session
                .shared_session_uid
                .as_deref()
                .unwrap_or(existing.session.session_uid.as_str())
                == imported_shared
        })
    });

    let target_index = target_index.or_else(|| {
        stored.iter().position(|existing| {
            session_uid_aliases(&existing.session)
                .iter()
                .any(|alias| alias == &imported.session_uid)
        })
    });

    if let Some(index) = target_index {
        let mut merged = merge_sessions(stored[index].session.clone(), imported);
        set_native_ref(&mut merged, backend, native_id, native_path);
        ensure_shared_session_uid(&mut merged);
        let stored_path = store_canonical_session(repo, &merged)?;
        stored[index] = StoredCanonical {
            session: merged.clone(),
        };
        return Ok((merged.session_uid, stored_path));
    }

    let stored_path = store_canonical_session(repo, &imported)?;
    stored.push(StoredCanonical {
        session: imported.clone(),
    });
    Ok((imported.session_uid, stored_path))
}

fn merge_sessions(mut anchor: SteadSession, incoming: SteadSession) -> SteadSession {
    let anchor_shared = anchor
        .shared_session_uid
        .clone()
        .unwrap_or_else(|| anchor.session_uid.clone());
    anchor.shared_session_uid = Some(anchor_shared);

    if incoming.session_uid != anchor.session_uid {
        add_session_uid_alias(&mut anchor, &incoming.session_uid);
    }
    for alias in session_uid_aliases(&incoming) {
        add_session_uid_alias(&mut anchor, &alias);
    }
    if let Some(shared) = incoming.shared_session_uid.as_ref()
        && shared != &anchor.session_uid
    {
        add_session_uid_alias(&mut anchor, shared);
    }

    if incoming.metadata.created_at < anchor.metadata.created_at {
        anchor.metadata.created_at = incoming.metadata.created_at;
    }
    if incoming.metadata.updated_at > anchor.metadata.updated_at {
        anchor.metadata.updated_at = incoming.metadata.updated_at;
    }
    if anchor.metadata.title.is_none() {
        anchor.metadata.title = incoming.metadata.title.clone();
    }
    if anchor.metadata.project_root == "/unknown" && incoming.metadata.project_root != "/unknown" {
        anchor.metadata.project_root = incoming.metadata.project_root.clone();
    }

    let mut source_files: Vec<String> = anchor
        .source
        .source_files
        .iter()
        .chain(incoming.source.source_files.iter())
        .cloned()
        .collect();
    source_files = dedupe_strings(source_files);
    anchor.source.source_files = source_files;
    upsert_backend_raw_lines(&mut anchor.raw_vendor_payload, &incoming);

    let mut index_by_key: HashMap<String, usize> = HashMap::new();
    let mut merged_events = Vec::new();
    for event in anchor.events.into_iter().chain(incoming.events.into_iter()) {
        let key = format!(
            "{}|{}|{}|{:?}",
            event.stream_id, event.event_uid, event.timestamp, event.kind
        );
        if let Some(index) = index_by_key.get(&key).copied() {
            merged_events[index] = event;
        } else {
            index_by_key.insert(key, merged_events.len());
            merged_events.push(event);
        }
    }
    stead_session_model::canonical_sort_events(&mut merged_events);
    anchor.events = merged_events;
    anchor
}

fn load_all_canonical_sessions(repo: &Path) -> Result<Vec<StoredCanonical>> {
    let store = canonical_store_dir(repo);
    if !store.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&store)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let mut session: SteadSession = serde_json::from_str(&raw)?;
        ensure_shared_session_uid(&mut session);
        out.push(StoredCanonical { session });
    }
    Ok(out)
}

fn canonical_store_dir(repo: &Path) -> PathBuf {
    repo.join(".stead-core").join("sessions")
}

fn canonical_session_path(repo: &Path, session_uid: &str) -> PathBuf {
    let sanitized: String = session_uid
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = if sanitized.is_empty() {
        format!("session-{}", short_hash(session_uid))
    } else {
        format!("{}-{}", sanitized, short_hash(session_uid))
    };
    canonical_store_dir(repo).join(format!("{}.json", file_name))
}

fn store_canonical_session(repo: &Path, session: &SteadSession) -> Result<PathBuf> {
    let path = canonical_session_path(repo, &session.session_uid);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(session)?)?;
    Ok(path)
}

fn load_canonical_session(repo: &Path, session_uid: &str) -> Result<SteadSession> {
    let direct = canonical_session_path(repo, session_uid);
    if direct.exists() {
        let raw = std::fs::read_to_string(&direct)?;
        return Ok(serde_json::from_str(&raw)?);
    }

    let store = canonical_store_dir(repo);
    if !store.exists() {
        return Err(anyhow!(
            "canonical store does not exist at {}",
            store.display()
        ));
    }
    for entry in std::fs::read_dir(&store)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let session: SteadSession = serde_json::from_str(&raw)?;
        if canonical_lookup_matches(&session, session_uid) {
            return Ok(session);
        }
    }
    Err(anyhow!("canonical session not found: {}", session_uid))
}

fn canonical_lookup_matches(session: &SteadSession, lookup: &str) -> bool {
    if session.session_uid == lookup {
        return true;
    }
    if session.shared_session_uid.as_deref() == Some(lookup) {
        return true;
    }
    session_uid_aliases(session)
        .iter()
        .any(|alias| alias == lookup)
}

fn ensure_shared_session_uid(session: &mut SteadSession) -> bool {
    if session.shared_session_uid.is_none() {
        session.shared_session_uid = Some(session.session_uid.clone());
        return true;
    }
    false
}

fn add_session_uid_alias(session: &mut SteadSession, alias: &str) {
    if alias.is_empty() || alias == session.session_uid {
        return;
    }
    let entry = session
        .extensions
        .entry("session_uid_aliases".to_string())
        .or_insert_with(|| json!([]));
    if !entry.is_array() {
        *entry = json!([]);
    }
    if let Some(arr) = entry.as_array_mut() {
        if arr.iter().any(|v| v.as_str() == Some(alias)) {
            return;
        }
        arr.push(Value::String(alias.to_string()));
    }
}

fn session_uid_aliases(session: &SteadSession) -> Vec<String> {
    session
        .extensions
        .get("session_uid_aliases")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            out.push(value);
        }
    }
    out
}

fn upsert_backend_raw_lines(anchor_raw: &mut Value, incoming: &SteadSession) {
    let Some(lines) = incoming
        .raw_vendor_payload
        .get("lines")
        .and_then(|v| v.as_array())
        .cloned()
    else {
        return;
    };

    if !anchor_raw.is_object() {
        *anchor_raw = json!({});
    }
    let Some(raw_obj) = anchor_raw.as_object_mut() else {
        return;
    };
    let backend_lines = raw_obj
        .entry("backend_lines".to_string())
        .or_insert_with(|| json!({}));
    if !backend_lines.is_object() {
        *backend_lines = json!({});
    }
    if let Some(map) = backend_lines.as_object_mut() {
        map.insert(
            incoming.source.backend.as_str().to_string(),
            Value::Array(lines),
        );
    }
}

fn set_native_ref(session: &mut SteadSession, backend: Backend, native_id: &str, path: &Path) {
    let key = backend_key(backend);
    let entry = session
        .extensions
        .entry("native_refs".to_string())
        .or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    if let Some(map) = entry.as_object_mut() {
        map.insert(
            key.to_string(),
            json!({
                "session_id": native_id,
                "path": path.display().to_string()
            }),
        );
    }
}

fn get_native_ref(session: &SteadSession, backend: Backend) -> Option<(String, PathBuf)> {
    let key = backend_key(backend);
    let refs = session.extensions.get("native_refs")?.as_object()?;
    let selected = refs.get(key)?.as_object()?;
    let session_id = selected.get("session_id")?.as_str()?.to_string();
    let path = selected.get("path")?.as_str()?.to_string();
    Some((session_id, PathBuf::from(path)))
}

fn choose_native_id(session: &SteadSession, backend: Backend) -> String {
    if let Some((id, _)) = get_native_ref(session, backend) {
        return id;
    }
    if source_backend_matches(session.source.backend, backend) {
        return session.source.original_session_id.clone();
    }
    deterministic_backend_uuid(session, backend).to_string()
}

fn deterministic_backend_uuid(session: &SteadSession, backend: Backend) -> Uuid {
    let shared = session
        .shared_session_uid
        .as_deref()
        .unwrap_or(session.session_uid.as_str());
    let key = format!("stead-native:{}:{}", backend_key(backend), shared);
    Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes())
}

fn source_backend_matches(source_backend: BackendKind, backend: Backend) -> bool {
    matches!(
        (source_backend, backend),
        (BackendKind::Codex, Backend::Codex) | (BackendKind::ClaudeCode, Backend::Claude)
    )
}

fn short_hash(value: &str) -> String {
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(value.as_bytes());
    format!("{:016x}", hasher.finish())[..8].to_string()
}

fn default_materialized_path(
    base_dir: &Path,
    repo: &Path,
    backend: Backend,
    native_id: &str,
) -> PathBuf {
    let now = Utc::now();
    let timestamp = now.format("%Y-%m-%dT%H-%M-%S").to_string();
    match backend {
        Backend::Codex => codex_sessions_root(base_dir)
            .join(format!("{:04}", now.year()))
            .join(format!("{:02}", now.month()))
            .join(format!("{:02}", now.day()))
            .join(format!("rollout-{timestamp}-{native_id}.jsonl")),
        Backend::Claude => {
            let slug = repo.display().to_string().replace(['/', '\\'], "-");
            base_dir
                .join("projects")
                .join(slug)
                .join(format!("{}.jsonl", native_id))
        }
    }
}

fn codex_sessions_root(base_dir: &Path) -> PathBuf {
    if base_dir
        .file_name()
        .is_some_and(|v| v.to_string_lossy().eq_ignore_ascii_case("sessions"))
    {
        base_dir.to_path_buf()
    } else {
        base_dir.join("sessions")
    }
}

fn prune_codex_rollouts_for_native_id(base_dir: &Path, native_id: &str, keep: &Path) -> Result<()> {
    let root = codex_sessions_root(base_dir);
    if !root.exists() {
        return Ok(());
    }
    prune_codex_rollouts_in_dir(&root, native_id, keep)
}

fn prune_codex_rollouts_in_dir(dir: &Path, native_id: &str, keep: &Path) -> Result<()> {
    let suffix = format!("-{native_id}.jsonl");
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            prune_codex_rollouts_in_dir(&path, native_id, keep)?;
            continue;
        }
        if !file_type.is_file() || path == keep {
            continue;
        }
        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if name.ends_with(&suffix) {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn backend_key(backend: Backend) -> &'static str {
    match backend {
        Backend::Codex => "codex",
        Backend::Claude => "claude",
    }
}

fn scope_sessions_to_repo(repo: &Path, sessions: Vec<NativeSessionRef>) -> Vec<NativeSessionRef> {
    let (repo_scoped, others): (Vec<_>, Vec<_>) = sessions
        .into_iter()
        .partition(|session| session_matches_repo(repo, session.project_root.as_deref()));
    if repo_scoped.is_empty() {
        others
    } else {
        repo_scoped
    }
}

fn session_matches_repo(repo: &Path, project_root: Option<&str>) -> bool {
    let Some(project_root) = project_root else {
        return false;
    };
    normalize_path(repo) == normalize_path(Path::new(project_root))
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
