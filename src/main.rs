use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use nostar::Error;
use nostar::config::Config;
use nostar::paths::Paths;

/// Exit code for an aborted command (TECHNIK.md 12.1).
const EXIT_ABORT: u8 = 2;

#[derive(Parser)]
#[command(
    version,
    about = "Displays German gender forms in the generic masculine by patching font copies"
)]
struct Cli {
    /// More output on stderr: -v info, -vv debug
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dictionary download
    Data {
        #[command(subcommand)]
        action: DataAction,
    },
    /// Rule list
    Rules {
        #[command(subcommand)]
        action: RulesAction,
    },
    /// Show how a text would be displayed
    Show { text: String },
    /// Patch fonts and activate them
    Apply {
        /// Only show what would happen
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the current state
    Status,
    /// Undo everything
    Remove {
        /// Keep configuration and downloaded data
        #[arg(long)]
        keep_config: bool,
    },
}

#[derive(Subcommand)]
enum DataAction {
    /// Download the dictionary in the configured version
    Update,
}

#[derive(Subcommand)]
enum RulesAction {
    /// Build the rule list from the dictionary
    Build,
    /// Check the rule list against the test cases
    Test,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Data {
                action: DataAction::Update,
            } => "data update",
            Self::Rules {
                action: RulesAction::Build,
            } => "rules build",
            Self::Rules {
                action: RulesAction::Test,
            } => "rules test",
            Self::Show { .. } => "show",
            Self::Apply { .. } => "apply",
            Self::Status => "status",
            Self::Remove { .. } => "remove",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let paths = match Paths::from_env() {
        Ok(p) => p,
        Err(e) => {
            // No logger without paths.
            eprintln!("error: paths: {e}");
            return ExitCode::from(EXIT_ABORT);
        }
    };
    if let Err(e) = nostar::log::init(cli.verbose, &paths.log(), &paths.log_backup()) {
        log::warn!(target: "log", "{}", describe(&e, &paths));
    }
    let name = cli.command.name();
    log::info!(target: "cmd", "start {name}");
    let start = Instant::now();
    let result = run(&cli.command, &paths);
    let elapsed = start.elapsed().as_millis();
    match result {
        Ok(code) => {
            log::info!(target: "cmd", "end {name} time={elapsed}ms");
            code
        }
        Err(e) => {
            log::error!(target: e.module(), "{}", describe(&e, &paths));
            log::info!(target: "cmd", "abort {name} time={elapsed}ms");
            ExitCode::from(EXIT_ABORT)
        }
    }
}

fn run(command: &Command, paths: &Paths) -> Result<ExitCode, Error> {
    let config = Config::load(&paths.config_file())?;
    match command {
        Command::Data {
            action: DataAction::Update,
        } => nostar::cmd::data::update(paths, &config)?,
        Command::Rules {
            action: RulesAction::Build,
        } => return nostar::cmd::rules::build(paths, &config),
        Command::Rules {
            action: RulesAction::Test,
        } => return nostar::cmd::rules::test(paths, &config),
        Command::Show { text } => nostar::cmd::show::show(paths, &config, text)?,
        Command::Apply { dry_run } => return nostar::cmd::apply::apply(paths, &config, *dry_run),
        Command::Status => nostar::cmd::status::status(paths, &config)?,
        Command::Remove { keep_config } => nostar::cmd::remove::remove(paths, *keep_config)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// Message plus `caused by:` lines, home directory shortened to `~`.
fn describe(e: &dyn std::error::Error, paths: &Paths) -> String {
    let mut text = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        text.push_str(&format!("\n  caused by: {c}"));
        cause = c.source();
    }
    paths.tilde(&text)
}
