use clap::{Parser, Subcommand};
use std::os::unix::net::UnixDatagram;
use tests::management::{ProgramLoad, ProgramRemove, SwitchManagementRequest};

#[derive(Parser)]
struct Cli {
    #[clap(long, default_value = "/tmp/switch-admin.sock")]
    socket: String,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ProgramLoad { program_id: u16 },
    ProgramRemove { program_id: u16 },
}

fn main() {
    let cli = Cli::parse();

    let req = match cli.command {
        Commands::ProgramLoad { program_id } => {
            SwitchManagementRequest::ProgramLoad(ProgramLoad { program_id })
        }
        Commands::ProgramRemove { program_id } => {
            SwitchManagementRequest::ProgramRemove(ProgramRemove { program_id })
        }
    };

    let sock = UnixDatagram::unbound().expect("Failed to create socket");
    sock.send_to(&serde_json::to_vec(&req).unwrap(), &cli.socket)
        .expect("Failed to send to switch_daemon");

    println!("sent {:?}", req);
}

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::os::unix::net::UnixDatagram;
use tests::management::{ProgramLoad, ProgramRemove, SwitchManagementRequest};

/// Admin CLI for controlling a running switch_daemon.
#[derive(Parser)]
struct Cli {
    /// Path to the switch_daemon's management socket.
    #[clap(long, default_value = "/tmp/switch-admin.sock")]
    socket: String,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Load a user_program by id.
    ProgramLoad { program_id: u16 },
    /// Remove a loaded user_program by id.
    ProgramRemove { program_id: u16 },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let req = match cli.command {
        Commands::ProgramLoad { program_id } => {
            SwitchManagementRequest::ProgramLoad(ProgramLoad { program_id })
        }
        Commands::ProgramRemove { program_id } => {
            SwitchManagementRequest::ProgramRemove(ProgramRemove { program_id })
        }
    };

    let sock = UnixDatagram::unbound().context("Failed to create socket")?;
    let payload = serde_json::to_vec(&req).context("Failed to serialize request")?;
    sock.send_to(&payload, &cli.socket)
        .with_context(|| format!("Failed to send to switch_daemon at {}", cli.socket))?;

    println!("sent {:?}", req);
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let req = match cli.command {
        Commands::ProgramLoad { program_id } => {
            SwitchManagementRequest::ProgramLoad(ProgramLoad { program_id })
        }
        Commands::ProgramRemove { program_id } => {
            SwitchManagementRequest::ProgramRemove(ProgramRemove { program_id })
        }
    };

    let sock = UnixDatagram::unbound().expect("failed to create socket");
    sock.send_to(&serde_json::to_vec(&req).unwrap(), &cli.socket)
        .expect("failed to send to switch_daemon");

    println!("sent {:?}", req);
}