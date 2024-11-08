use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    author,
    version,
    about,
    help_template = "{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}",
    arg_required_else_help = true
)]
pub struct Args {
    /// Unimplemented
    #[arg(short, long)]
    pub verbose: bool,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install packages
    #[command(arg_required_else_help = true)]
    #[clap(name = "install", visible_alias = "i", visible_alias = "add")]
    Install {
        /// Packages to install
        #[arg(required = true)]
        packages: Vec<String>,

        /// Whether to force install the package
        #[arg(required = false, short, long)]
        force: bool,

        /// Skip all prompts and use first
        #[arg(required = false, short, long)]
        yes: bool,

        /// Set portable dir for home & config
        #[arg(required = false, short, long, num_args = 0..=1)]
        portable: Option<Option<String>>,

        /// Set portable home
        #[arg(required = false, long, num_args = 0..=1)]
        portable_home: Option<Option<String>>,

        /// Set portable config
        #[arg(required = false, long, num_args = 0..=1)]
        portable_config: Option<Option<String>>,
    },

    /// Search package
    #[command(arg_required_else_help = true)]
    #[clap(name = "search", visible_alias = "s", visible_alias = "find")]
    Search {
        /// Query to search
        #[arg(required = true)]
        query: String,

        /// Case sensitive search
        #[arg(required = false, long, alias = "exact")]
        case_sensitive: bool,

        /// Limit number of result
        #[arg(required = false, long)]
        limit: Option<usize>,
    },

    /// Query package info
    #[command(arg_required_else_help = true)]
    #[clap(name = "query", visible_alias = "Q")]
    Query {
        /// Package to query
        #[arg(required = true)]
        query: String,
    },

    /// Remove packages
    #[command(arg_required_else_help = true)]
    #[clap(name = "remove", visible_alias = "r", visible_alias = "del")]
    Remove {
        /// Packages to remove
        #[arg(required = true)]
        packages: Vec<String>,

        /// Remove exact package only
        #[arg(required = false, long, short)]
        exact: bool,
    },

    /// Sync with remote metadata
    #[clap(name = "sync", visible_alias = "S", visible_alias = "fetch")]
    Sync,

    /// Update packages
    #[clap(name = "update", visible_alias = "u", visible_alias = "upgrade")]
    Update {
        /// Packages to update
        #[arg(required = false)]
        packages: Option<Vec<String>>,
    },

    /// Show info about installed packages
    #[clap(name = "info", visible_alias = "list-installed")]
    ListInstalledPackages {
        /// Packages to get info about
        #[arg(required = false)]
        packages: Option<Vec<String>>,
    },

    /// List all available packages
    #[clap(name = "list", visible_alias = "ls")]
    ListPackages {
        /// Which collection to get the packages from
        #[arg(required = false)]
        collection: Option<String>,
    },

    /// Inspect package build log
    #[command(arg_required_else_help = true)]
    #[clap(name = "log")]
    Log {
        /// Package to view log for
        #[arg(required = true)]
        package: String,
    },

    /// Inspect package build script
    #[command(arg_required_else_help = true)]
    #[clap(name = "inspect")]
    Inspect {
        /// Package to view build script for
        #[arg(required = true)]
        package: String,
    },

    /// Run packages without installing to PATH
    #[command(arg_required_else_help = true)]
    #[clap(name = "run", visible_alias = "exec", visible_alias = "execute")]
    Run {
        /// Skip all prompts and use first
        #[arg(required = false, short, long)]
        yes: bool,

        /// Command to execute
        #[arg(required = true, trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Use package from different family
    #[command(arg_required_else_help = true)]
    #[clap(name = "use")]
    Use {
        /// The package to use
        #[arg(required = true)]
        package: String,
    },

    /// Download arbitrary files
    #[command(arg_required_else_help = true)]
    #[clap(name = "download", visible_alias = "dl")]
    Download {
        /// Links to files
        #[arg(required = true)]
        links: Vec<String>,

        /// Skip all prompts and use first
        #[arg(required = false, short, long)]
        yes: bool,

        /// Output file path
        #[arg(required = false, short, long)]
        output: Option<String>,
    },

    /// Health check
    #[clap(name = "health")]
    Health,

    /// Generate default config
    #[clap(name = "defconfig")]
    DefConfig,
}
