use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version)]
#[command(arg_required_else_help = true)]
pub struct Args {
    #[arg(short, long)]
    pub verbose: bool,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install packages
    #[command(arg_required_else_help = true)]
    #[clap(name = "install", visible_alias = "i")]
    Install {
        #[arg(required = true)]
        packages: Vec<String>,

        #[arg(required = false)]
        #[arg(short, long)]
        force: bool,
    },

    #[command(arg_required_else_help = true)]
    #[clap(name = "search", visible_alias = "s")]
    Search {
        #[arg(required = true)]
        query: String,
    },

    /// Remove packages
    #[command(arg_required_else_help = true)]
    #[clap(name = "remove", visible_alias = "r")]
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    /// Fetch and update metadata
    #[command(name = "fetch")]
    Fetch,

    /// Update packages
    #[command(name = "update", visible_alias = "u")]
    Update {
        #[arg(required = false)]
        package: Option<Vec<String>>,
    },

    /// List installed packages
    #[command(name = "list")]
    ListPackages,
}
