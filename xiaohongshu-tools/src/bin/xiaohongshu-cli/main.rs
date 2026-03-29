use clap::{Parser, Subcommand};
use xiaohongshu_tools::auth;

#[derive(Parser)]
#[command(name = "xhs")]
#[command(about = "Xiaohongshu CLI tools", long_about = None)]
struct Cli {
    #[arg(long, global = true)]
    headless: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    Login,
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Login => {
                auth::login().await?;
            }
            AuthCommands::Status => {
                let logged_in = auth::check_status(cli.headless).await?;
                if logged_in {
                    println!("Logged in");
                } else {
                    println!("Not logged in");
                }
            }
        },
    }

    Ok(())
}
