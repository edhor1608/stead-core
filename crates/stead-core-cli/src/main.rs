use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use stead_session_adapters::claude::ClaudeAdapter;
use stead_session_adapters::codex::CodexAdapter;
use stead_session_model::SteadSession;

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
    }
}

fn run_list(backend: Backend, base_dir: PathBuf, json: bool) -> Result<()> {
    let sessions = match backend {
        Backend::Codex => CodexAdapter::from_base_dir(base_dir).list_sessions()?,
        Backend::Claude => ClaudeAdapter::from_base_dir(base_dir).list_sessions()?,
    };
    if json {
        println!("{}", serde_json::to_string(&sessions)?);
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
    std::fs::write(&out, serialized)
        .with_context(|| format!("failed to write canonical session to {}", out.display()))?;
    Ok(())
}

fn run_export(to: Backend, base_dir: PathBuf, input: PathBuf, out: PathBuf) -> Result<()> {
    let raw = std::fs::read_to_string(&input)
        .with_context(|| format!("failed to read canonical input {}", input.display()))?;
    let session: SteadSession = serde_json::from_str(&raw)
        .with_context(|| format!("invalid canonical JSON in {}", input.display()))?;
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
