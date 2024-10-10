use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version)]
#[command(arg_required_else_help = true)]
pub struct Args {
    /// Unimplemented
    #[arg(short, long)]
    pub verbose: bool,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install packages; supports '--force' flag
    #[command(arg_required_else_help = true)]
    #[clap(name = "install", visible_alias = "i", alias = "add")]
    Install {
        /// Packages to install
        #[arg(required = true)]
        packages: Vec<String>,

        /// Whether to force install the package
        #[arg(required = false)]
        #[arg(short, long)]
        force: bool,
    },

    /// Search package
    #[command(arg_required_else_help = true)]
    #[clap(name = "search", visible_alias = "s", alias = "find")]
    Search {
        /// Query to search
        #[arg(required = true)]
        query: String,
    },

    /// Query package info
    #[command(arg_required_else_help = true)]
    #[clap(name = "query", visible_alias = "Q")]
    Query {
        /// Package to inspect
        #[arg(required = true)]
        query: String,
    },

    /// Remove packages
    #[command(arg_required_else_help = true)]
    #[clap(name = "remove", visible_alias = "r", alias = "del")]
    Remove {
        /// Packages to remove
        #[arg(required = true)]
        packages: Vec<String>,
    },

    /// Sync with remote registry
    #[clap(name = "sync", visible_alias = "S", alias = "fetch")]
    Sync,

    /// Update packages
    #[clap(name = "update", visible_alias = "u", alias = "upgrade")]
    Update {
        /// Packages to update
        #[arg(required = false)]
        packages: Option<Vec<String>>,
    },

    /// Show info about installed packages
    #[clap(name = "info", alias = "list-installed")]
    ListInstalledPackages {
        /// Packages to get info about
        #[arg(required = false)]
        packages: Option<Vec<String>>,
    },

    /// List all available packages
    #[clap(name = "list", alias = "ls")]
    ListPackages {
        /// Root path of packages
        #[arg(required = false)]
        root_path: Option<String>,
    },

    /// Inspect package build log
    #[command(arg_required_else_help = true)]
    #[clap(name = "inspect", alias = "log")]
    Inspect {
        /// Package to inspect
        #[arg(required = true)]
        package: String,
    },

    /// Run packages without installing to PATH
    #[command(arg_required_else_help = true)]
    #[clap(name = "run", visible_alias = "exec", alias = "execute")]
    Run {
        /// Command to execute
        #[arg(required = true)]
        command: Vec<String>,
    },

    /// Use different variant of a package
    #[command(arg_required_else_help = true)]
    #[clap(name = "use")]
    Use {
        /// The package to use
        #[arg(required = true)]
        package: String,
    },
}
