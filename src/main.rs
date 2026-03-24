use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "phalus", about = "Private Headless Automated License Uncoupling System")]
#[command(version, propagate_version = true)]
struct Cli {
    /// Increase verbosity (can be repeated)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a manifest and plan which packages to replace
    Plan {
        /// Path to the manifest file (package.json, requirements.txt, Cargo.toml, …)
        #[arg(value_name = "MANIFEST")]
        manifest: String,

        /// Output plan as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run the full pipeline for all packages in a manifest
    Run {
        /// Path to the manifest file
        #[arg(value_name = "MANIFEST")]
        manifest: String,

        /// Target language for the replacement implementation
        #[arg(long, default_value = "same")]
        lang: String,

        /// Isolation mode (context | process | container)
        #[arg(long, default_value = "context")]
        isolation: String,

        /// Maximum number of packages to process in parallel
        #[arg(long, default_value_t = 1)]
        parallelism: usize,

        /// Skip validation step
        #[arg(long)]
        no_validate: bool,

        /// Output directory for generated replacements
        #[arg(short, long, default_value = "phalus-out")]
        output: String,
    },

    /// Run the pipeline for a single package
    RunOne {
        /// Package name
        #[arg(value_name = "PACKAGE")]
        package: String,

        /// Package version (or version constraint)
        #[arg(value_name = "VERSION")]
        version: String,

        /// Ecosystem (npm | pypi | crates | go)
        #[arg(long, default_value = "npm")]
        ecosystem: String,

        /// Target language for the replacement implementation
        #[arg(long, default_value = "same")]
        lang: String,

        /// Isolation mode (context | process | container)
        #[arg(long, default_value = "context")]
        isolation: String,

        /// Output directory for generated replacements
        #[arg(short, long, default_value = "phalus-out")]
        output: String,
    },

    /// Inspect cached docs / CSP spec for a package
    Inspect {
        /// Package name
        #[arg(value_name = "PACKAGE")]
        package: String,

        /// Package version
        #[arg(value_name = "VERSION")]
        version: String,

        /// Ecosystem (npm | pypi | crates | go)
        #[arg(long, default_value = "npm")]
        ecosystem: String,

        /// Show the full CSP spec rather than metadata
        #[arg(long)]
        spec: bool,
    },

    /// Validate a previously generated replacement
    Validate {
        /// Path to the output directory produced by run / run-one
        #[arg(value_name = "OUTPUT_DIR")]
        output_dir: String,

        /// Fail if overall similarity score is above this threshold (0.0–1.0)
        #[arg(long, default_value_t = 0.3)]
        similarity_threshold: f64,
    },

    /// Manage phalus configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,

    /// Set a configuration key
    Set {
        /// Configuration key (e.g. llm.api_key)
        key: String,
        /// Value to set
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialise tracing based on verbosity
    let level = match cli.verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    match cli.command {
        Commands::Plan { .. } => {
            todo!("plan command")
        }
        Commands::Run { .. } => {
            todo!("run command")
        }
        Commands::RunOne { .. } => {
            todo!("run-one command")
        }
        Commands::Inspect { .. } => {
            todo!("inspect command")
        }
        Commands::Validate { .. } => {
            todo!("validate command")
        }
        Commands::Config { action } => match action {
            ConfigAction::Show => todo!("config show"),
            ConfigAction::Set { .. } => todo!("config set"),
            ConfigAction::Get { .. } => todo!("config get"),
        },
    }
}
