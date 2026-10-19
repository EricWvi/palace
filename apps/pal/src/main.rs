//! `pal`, the Palace command line. It reaches Palace only through the public HTTP API, with the
//! address and access token taken from the environment so the token never appears in argv.
mod client;
mod config;
mod failure;
mod jottings;
mod update;

use clap::{Parser, Subcommand, error::ErrorKind};
use client::Client;
use config::Config;
use failure::Failure;
use std::{path::PathBuf, process::ExitCode};
use uuid::Uuid;

const VERSION: &str = env!("PAL_BUILD_VERSION");

/// Palace CLI; Palace API commands read PAL_URL and PAL_TOKEN (an access token from 密钥管理).
#[derive(Parser)]
#[command(name = "pal", version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a UUID v7 locally; needs no PAL_URL or PAL_TOKEN.
    Uuid,
    /// Download the latest pal release from GitHub and replace this executable.
    Update,
    /// Media files kept in Palace's object storage, referenced only by their media id.
    #[command(subcommand)]
    S3(S3),
    /// 札记 kept as Markdown files in a Git repository.
    #[command(subcommand)]
    Jottings(Jottings),
}

#[derive(Subcommand)]
enum Jottings {
    /// Check a jottings folder the way the server checks a commit; needs no PAL_URL or PAL_TOKEN.
    Check {
        #[arg(value_name = "FOLDER")]
        folder: PathBuf,
    },
}

#[derive(Subcommand)]
enum S3 {
    /// Upload one file and print the stored media as one line of JSON.
    Upload {
        #[arg(value_name = "PATH_TO_FILE")]
        path: PathBuf,
    },
    /// Delete a media file by its id.
    Delete {
        #[arg(value_name = "UUID_V7")]
        id: Uuid,
    },
}

/// Usage errors exit 2 from the parser; every other failure exits 1 with stdout left empty.
fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // Print the repository identity verbatim instead of clap's binary-name/version prefix.
        Err(error) if error.kind() == ErrorKind::DisplayVersion => {
            println!("{VERSION}");
            return ExitCode::SUCCESS;
        }
        Err(error) => error.exit(),
    };
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("pal: {failure}");
            ExitCode::FAILURE
        }
    }
}

/// Resolves Palace credentials only for API commands; local commands and updates never read them.
fn run(command: Command) -> Result<(), Failure> {
    match command {
        Command::Uuid => println!("{}", Uuid::now_v7()),
        Command::Update => println!("Updated pal to {}", update::run()?),
        Command::S3(command) => {
            let client = Client::new(Config::from_env(std::env::var_os)?)?;
            match command {
                S3::Upload { path } => println!("{}", client.upload(&path)?),
                S3::Delete { id } => client.delete(id)?,
            }
        }
        Command::Jottings(Jottings::Check { folder }) => {
            jottings::check(&folder)?;
        }
    }
    Ok(())
}
