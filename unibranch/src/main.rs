use anyhow::Context;
use clap::{command, Parser, Subcommand};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use ubr::{
    commands::{commit, create, push, sync},
    git::{CommandOption, GitRepo},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    quiet: bool,

    #[arg(short, long)]
    dry_run: bool,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    Create(create::Options),
    Sync(sync::Options),
    Push,
    Amend(commit::Options),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let level = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    let subscriber = tracing_subscriber::fmt()
        .pretty()
        .with_file(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .without_time()
        .with_target(false)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(level.into())
                .from_env_lossy(),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let remote_option = if cli.dry_run {
        CommandOption::DryRun
    } else if cli.quiet {
        CommandOption::Silent
    } else {
        CommandOption::Default
    };
    let git_repo = GitRepo::open_with_remote(".", remote_option).context("Opening GIT repo")?;

    match cli.command {
        Commands::Create(config) => create::execute(config, git_repo)?,
        Commands::Sync(config) => sync::execute(config, git_repo)?,
        Commands::Push => push::execute(".")?,
        Commands::Amend(config) => commit::execute(config, git_repo)?,
    };
    Ok(())
}
