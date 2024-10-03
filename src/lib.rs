use clap::Parser;
use cli::{Args, Commands};

mod cli;
mod core;

pub async fn init() {
    let args = Args::parse();

    match args.command {
        Commands::Install { packages, force } => {
            todo!()
        }
        Commands::Fetch => {
            todo!()
        }
        Commands::Remove { packages } => todo!(),
        Commands::Update { package } => todo!(),
        Commands::ListPackages => todo!(),
    };
}
