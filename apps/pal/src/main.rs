//! `pal`, the Palace command line. It reaches Palace only through the public HTTP API, with the
//! address and access token taken from the environment so the token never appears in argv.
mod client;
mod config;
mod failure;

use clap::{Parser, Subcommand};
use client::Client;
use config::Config;
use failure::Failure;
use std::{path::PathBuf, process::ExitCode};
use uuid::Uuid;

/// Reads PAL_URL (the Palace address) and PAL_TOKEN (an access token from 密钥管理).
#[derive(Parser)]
#[command(name = "pal", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Media files kept in Palace's object storage, referenced only by their media id.
    #[command(subcommand)]
    S3(S3),
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
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("pal: {failure}");
            ExitCode::FAILURE
        }
    }
}

/// Resolves the environment before any network access, so a misconfiguration sends nothing.
fn run(command: Command) -> Result<(), Failure> {
    let client = Client::new(Config::from_env(std::env::var_os)?)?;
    match command {
        Command::S3(S3::Upload { path }) => println!("{}", client.upload(&path)?),
        Command::S3(S3::Delete { id }) => client.delete(id)?,
    }
    Ok(())
}
