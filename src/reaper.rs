use std::io::{self, Write};
use std::process::Command;
use std::time::Instant;

use cellar_sandbox::EnvVar;
use log::info;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T, E = std::io::Error> = std::result::Result<T, E>;

#[derive(Debug, Serialize, Deserialize)]
pub enum ReaperCommand {
    Execute {
        exec: String,
        args: Vec<String>,
        env: Vec<EnvVar>,
    },
}

impl ReaperCommand {
    pub fn dispatch<T: Write>(self, writable: T) -> bincode::Result<()> {
        bincode::serialize_into(writable, &self)
    }
}

#[derive(Error, Debug)]
pub enum ReaperError {}

fn start_logging() -> Result<()> {
    use flexi_logger::Logger;

    Logger::try_with_str("debug").unwrap().start().unwrap();

    Ok(())
}

// Suppress this main not being called, which also lets the other functions here not show as unused
#[allow(dead_code)]
fn main() -> Result<()> {
    start_logging()?;

    let start = Instant::now();
    info!("Reaper starting...");

    info!("Obtaining stdin lock");
    let stdin = io::stdin();
    let stdin = stdin.lock();

    info!("Listening for commands");
    let s: ReaperCommand = bincode::deserialize_from(stdin).unwrap();

    info!("Received Command {:#?}", s);

    match s {
        ReaperCommand::Execute { exec, args, .. } => {
            Command::new(exec).args(args).status().unwrap()
        }
    };

    info!(
        "Reaper shutting down! Ran for {:?}",
        Instant::now().duration_since(start)
    );
    Ok(())
}
