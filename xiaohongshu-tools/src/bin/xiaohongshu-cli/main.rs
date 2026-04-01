use clap::{Parser, Subcommand};
use xiaohongshu_tools::auth;
use xiaohongshu_tools::browser::BrowserOptions;

#[derive(Parser)]
#[command(name = "xhs")]
#[command(about = "Xiaohongshu CLI tools", long_about = None)]
struct Cli {
    #[arg(long, global = true)]
    headless: bool,

    #[arg(long, global = true)]
    proxy: Option<String>,

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
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,chromiumoxide=error")),
        )
        .init();

    let cli = Cli::parse();

    let opts = BrowserOptions {
        headless: cli.headless,
        proxy: cli.proxy,
    };

    match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Login => {
                auth::login(&opts).await?;
            }
            AuthCommands::Status => {
                let logged_in = auth::check_status(&opts).await?;
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
