use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use xiaohongshu_tools::auth;
use xiaohongshu_tools::browser::BrowserOptions;
use xiaohongshu_tools::commands::browse;
use xiaohongshu_tools::cookies;
use xiaohongshu_tools::human::ScrollSpeed;
use xiaohongshu_tools::i18n;
use xiaohongshu_tools::output::{Format, Output};

#[derive(Parser)]
#[command(name = "xhs")]
struct Cli {
    #[arg(long, global = true)]
    headless: bool,

    #[arg(long, global = true)]
    proxy: Option<String>,

    #[arg(long, global = true, default_value = "text")]
    format: String,

    #[arg(long, global = true)]
    lang: Option<String>,

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
        #[arg(long, value_delimiter = ',')]
        keywords: Vec<String>,

        #[arg(long, value_delimiter = ',')]
        exclude: Vec<String>,

        #[arg(long)]
        max_posts: Option<usize>,

        #[arg(long, default_value = "normal")]
        scroll_speed: String,

        #[arg(long)]
        interact: bool,

        #[arg(long)]
        duration: Option<u64>,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    Login,
    Logout,
    Status,
}

fn build_command() -> clap::Command {
    let cmd = Cli::command()
        .about(i18n::cli_about())
        .mut_arg("headless", |a| a.help(i18n::cli_headless_help()))
        .mut_arg("proxy", |a| a.help(i18n::cli_proxy_help()))
        .mut_arg("format", |a| a.help(i18n::cli_format_help()))
        .mut_arg("lang", |a| a.help(i18n::cli_lang_help()));

    let cmd = cmd.mut_subcommand("auth", |s| {
        s.about(i18n::cli_auth_about())
            .mut_subcommand("login", |s| s.about(i18n::cli_auth_login_about()))
            .mut_subcommand("logout", |s| s.about(i18n::cli_auth_logout_about()))
            .mut_subcommand("status", |s| s.about(i18n::cli_auth_status_about()))
    });
    cmd.mut_subcommand("browse", |s| {
        s.about(i18n::cli_browse_about())
            .mut_arg("keywords", |a| a.help(i18n::cli_keywords_help()))
            .mut_arg("exclude", |a| a.help(i18n::cli_exclude_help()))
            .mut_arg("max_posts", |a| a.help(i18n::cli_max_posts_help()))
            .mut_arg("scroll_speed", |a| a.help(i18n::cli_scroll_speed_help()))
            .mut_arg("interact", |a| a.help(i18n::cli_interact_help()))
            .mut_arg("duration", |a| a.help(i18n::cli_duration_help()))
    })
}

fn detect_pre_lang() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == "--lang")
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--lang="))
                .map(|a| a.trim_start_matches("--lang=").to_string())
        })
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

    let pre_lang = detect_pre_lang();
    i18n::init(pre_lang.as_deref());

    let command = build_command();
    let matches = command.get_matches();

    let cli = Cli::from_arg_matches(&matches)?;

    let format: Format = cli.format.parse()?;
    let out = Output::new(format);

    let opts = BrowserOptions {
        headless: cli.headless,
        proxy: cli.proxy,
    };

    match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Login => {
                let result = auth::login(&opts).await?;
                out.result(&result);
            }
            AuthCommands::Logout => {
                cookies::delete_cookies()?;
                out.result(&auth::LogoutResult { logged_out: true });
            }
            AuthCommands::Status => {
                let result = auth::check_status(&opts).await?;
                out.result(&result);
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
            let result = browse::run(
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
            out.result(&result);
        }
    }

    Ok(())
}
