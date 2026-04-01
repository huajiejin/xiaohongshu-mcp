use clap::{Parser, Subcommand};
use tracing::info;
use xiaohongshu_tools::auth;
use xiaohongshu_tools::browser::BrowserOptions;
use xiaohongshu_tools::commands::browse;
use xiaohongshu_tools::cookies;
use xiaohongshu_tools::human::ScrollSpeed;

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

    Browse {
        #[arg(
            long,
            value_delimiter = ',',
            help = "Only interact with posts matching these keywords"
        )]
        keywords: Vec<String>,

        #[arg(
            long,
            value_delimiter = ',',
            help = "Skip posts matching these keywords"
        )]
        exclude: Vec<String>,

        #[arg(long, help = "Stop after N matched posts")]
        max_posts: Option<usize>,

        #[arg(
            long,
            default_value = "normal",
            help = "Scroll speed: slow, normal, fast"
        )]
        scroll_speed: String,

        #[arg(long, help = "Click into posts and scroll comments")]
        interact: bool,

        #[arg(long, help = "Auto-exit after N seconds")]
        duration: Option<u64>,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    Login,
    Logout,
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,chromiumoxide=error")),
        )
        .with_target(false)
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
            AuthCommands::Logout => {
                cookies::delete_cookies()?;
                info!("Logged out");
            }
            AuthCommands::Status => {
                let logged_in = auth::check_status(&opts).await?;
                if logged_in {
                    info!("Logged in");
                } else {
                    info!("Not logged in");
                }
            }
        },
        Commands::Browse {
            keywords,
            exclude,
            max_posts,
            scroll_speed,
            interact,
            duration,
        } => {
            let speed: ScrollSpeed = scroll_speed.parse()?;
            browse::run(
                &browse::BrowseOptions {
                    keywords,
                    exclude,
                    max_posts,
                    scroll_speed: speed,
                    interact,
                    duration,
                },
                &opts,
            )
            .await?;
        }
    }

    Ok(())
}
