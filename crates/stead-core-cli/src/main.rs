use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use twox_hash::XxHash64;

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

fn run_sync(repo: PathBuf, codex_base: PathBuf, claude_base: PathBuf, json_out: bool) -> Result<()> {
    std::fs::create_dir_all(canonical_store_dir(&repo))?;

    let mut imported = Vec::new();

    let codex = CodexAdapter::from_base_dir(codex_base);
    for native in codex.list_sessions()? {
        let mut session = codex.import_from_file(&native.file_path)?;
        set_native_ref(&mut session, Backend::Codex, &native.native_id, &native.file_path);
        let stored = store_canonical_session(&repo, &session)?;
        imported.push(json!({
            "backend": "codex",
            "native_id": native.native_id,
            "session_uid": session.session_uid,
            "stored_at": stored
        }));
    }

    let claude = ClaudeAdapter::from_base_dir(claude_base);
    for native in claude.list_sessions()? {
        let mut session = claude.import_session(&native.native_id)?;
        set_native_ref(&mut session, Backend::Claude, &native.native_id, &native.file_path);
        let stored = store_canonical_session(&repo, &session)?;
        imported.push(json!({
            "backend": "claude",
            "native_id": native.native_id,
            "session_uid": session.session_uid,
            "stored_at": stored
        }));
    }

    if json_out {
        println!("{}", serde_json::to_string(&imported)?);
    } else {
        println!("synced {} sessions into {}", imported.len(), canonical_store_dir(&repo).display());
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
    let native_id = choose_native_id(&session, to);
    let output_path = out.unwrap_or_else(|| default_materialized_path(&base_dir, &repo, to, &native_id));
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut export_session = session.clone();
    export_session.source.original_session_id = native_id.clone();

    let report = match to {
        Backend::Codex => CodexAdapter::from_base_dir(base_dir).export_session(&export_session, &output_path)?,
        Backend::Claude => ClaudeAdapter::from_base_dir(base_dir).export_session(&export_session, &output_path)?,
    };

    set_native_ref(&mut session, to, &native_id, &output_path);
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

    let (native_id, native_path) = if let Some(found) = get_native_ref(&session, backend) {
        found
    } else {
        let Some(base_dir) = base_dir else {
            bail!("missing native projection for backend `{}`; provide --base-dir to materialize", backend_key(backend));
        };
        let native_id = choose_native_id(&session, backend);
        let output_path = out.unwrap_or_else(|| default_materialized_path(&base_dir, &repo, backend, &native_id));
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut export_session = session.clone();
        export_session.source.original_session_id = native_id.clone();
        match backend {
            Backend::Codex => {
                CodexAdapter::from_base_dir(base_dir).export_session(&export_session, &output_path)?;
            }
            Backend::Claude => {
                ClaudeAdapter::from_base_dir(base_dir).export_session(&export_session, &output_path)?;
            }
        }
        set_native_ref(&mut session, backend, &native_id, &output_path);
        store_canonical_session(&repo, &session)?;
        (native_id, output_path)
    };

    let status = if let Ok(runner) = std::env::var("STEAD_CORE_RUNNER") {
        Command::new(runner)
            .args([backend_key(backend), "--resume", &native_id, prompt])
            .status()?
    } else {
        let bin = match backend {
            Backend::Codex => std::env::var("STEAD_CORE_CODEX_BIN").unwrap_or_else(|_| "codex".to_string()),
            Backend::Claude => std::env::var("STEAD_CORE_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string()),
        };
        Command::new(bin)
            .args(["--resume", &native_id, prompt])
            .status()?
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

    Ok(())
}

fn canonical_store_dir(repo: &Path) -> PathBuf {
    repo.join(".stead-core").join("sessions")
}

fn canonical_session_path(repo: &Path, session_uid: &str) -> PathBuf {
    let sanitized: String = session_uid
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
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
        return Err(anyhow!("canonical store does not exist at {}", store.display()));
    }
    for entry in std::fs::read_dir(&store)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let session: SteadSession = serde_json::from_str(&raw)?;
        if session.session_uid == session_uid {
            return Ok(session);
        }
    }
    Err(anyhow!("canonical session not found: {}", session_uid))
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
    format!("bridge-{}-{}", backend_key(backend), short_hash(&session.session_uid))
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

fn default_materialized_path(base_dir: &Path, repo: &Path, backend: Backend, native_id: &str) -> PathBuf {
    let unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match backend {
        Backend::Codex => base_dir
            .join("sessions")
            .join("auto")
            .join("auto")
            .join("auto")
            .join(format!("rollout-{}-{}.jsonl", unix_secs, native_id)),
        Backend::Claude => {
            let slug = repo.display().to_string().replace(['/', '\\'], "-");
            base_dir.join("projects").join(slug).join(format!("{}.jsonl", native_id))
        }
    }
}

fn backend_key(backend: Backend) -> &'static str {
    match backend {
        Backend::Codex => "codex",
        Backend::Claude => "claude",
    }
}
