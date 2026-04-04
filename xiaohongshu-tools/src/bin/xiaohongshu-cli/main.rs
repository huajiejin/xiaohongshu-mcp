use std::time::Duration;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use xiaohongshu_tools::auth;
use xiaohongshu_tools::browser::BrowserOptions;
use xiaohongshu_tools::browser::human::ScrollSpeed;
use xiaohongshu_tools::browser::{create_browser, create_page_with_cookies};
use xiaohongshu_tools::commands::{creator, explore, note, search};
use xiaohongshu_tools::shared::i18n;
use xiaohongshu_tools::shared::output::{Format, Output};

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

    Explore {
        #[arg(long, value_delimiter = ',')]
        keywords: Vec<String>,

        #[arg(long, value_delimiter = ',')]
        exclude: Vec<String>,

        #[arg(long, default_value_t = 10)]
        max_notes: usize,

        #[arg(long, default_value = "normal")]
        scroll_speed: String,

        #[arg(long)]
        interact: bool,

        #[arg(long, default_value_t = 10)]
        duration: u64,
    },

    Search {
        query: String,

        #[arg(long)]
        sort_by: Option<String>,

        #[arg(long)]
        note_type: Option<String>,

        #[arg(long)]
        publish_time: Option<String>,

        #[arg(long)]
        search_scope: Option<String>,

        #[arg(long)]
        location: Option<String>,

        #[arg(long, default_value_t = 10)]
        max_notes: usize,

        #[arg(long, default_value = "normal")]
        scroll_speed: String,

        #[arg(long, default_value_t = 10)]
        duration: u64,
    },

    Creator {
        url: String,

        #[arg(long, default_value_t = 10)]
        max_notes: usize,

        #[arg(long, default_value = "normal")]
        scroll_speed: String,

        #[arg(long, default_value_t = 10)]
        duration: u64,
    },

    Note {
        url: String,

        #[arg(long, default_value_t = 10)]
        max_comments: usize,

        #[arg(long, default_value_t = 10)]
        max_replies: usize,

        #[arg(long, default_value = "normal")]
        scroll_speed: String,

        #[arg(long, default_value_t = 10)]
        duration: u64,
    },

    Open {
        url: String,
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
    cmd.mut_subcommand("explore", |s| {
        s.about(i18n::cli_explore_about())
            .mut_arg("keywords", |a| a.help(i18n::cli_keywords_help()))
            .mut_arg("exclude", |a| a.help(i18n::cli_exclude_help()))
            .mut_arg("max_notes", |a| a.help(i18n::cli_max_notes_help()))
            .mut_arg("scroll_speed", |a| a.help(i18n::cli_scroll_speed_help()))
            .mut_arg("interact", |a| a.help(i18n::cli_interact_help()))
            .mut_arg("duration", |a| a.help(i18n::cli_duration_help()))
    })
    .mut_subcommand("search", |s| {
        s.about(i18n::cli_search_about())
            .mut_arg("query", |a| a.help(i18n::cli_query_help()))
            .mut_arg("sort_by", |a| a.help(i18n::cli_sort_by_help()))
            .mut_arg("note_type", |a| a.help(i18n::cli_note_type_help()))
            .mut_arg("publish_time", |a| a.help(i18n::cli_publish_time_help()))
            .mut_arg("search_scope", |a| a.help(i18n::cli_search_scope_help()))
            .mut_arg("location", |a| a.help(i18n::cli_location_help()))
            .mut_arg("max_notes", |a| a.help(i18n::cli_max_notes_help()))
            .mut_arg("scroll_speed", |a| a.help(i18n::cli_scroll_speed_help()))
            .mut_arg("duration", |a| a.help(i18n::cli_duration_help()))
    })
    .mut_subcommand("creator", |s| {
        s.about(i18n::cli_creator_about())
            .mut_arg("url", |a| a.help(i18n::cli_url_help()))
            .mut_arg("max_notes", |a| a.help(i18n::cli_max_notes_help()))
            .mut_arg("scroll_speed", |a| a.help(i18n::cli_scroll_speed_help()))
            .mut_arg("duration", |a| a.help(i18n::cli_duration_help()))
    })
    .mut_subcommand("note", |s| {
        s.about(i18n::cli_note_about())
            .mut_arg("url", |a| a.help(i18n::cli_note_url_help()))
            .mut_arg("max_comments", |a| a.help(i18n::cli_max_comments_help()))
            .mut_arg("max_replies", |a| a.help(i18n::cli_max_replies_help()))
            .mut_arg("scroll_speed", |a| a.help(i18n::cli_scroll_speed_help()))
            .mut_arg("duration", |a| a.help(i18n::cli_duration_help()))
    })
    .mut_subcommand("open", |s| {
        s.about(i18n::cli_open_about())
            .mut_arg("url", |a| a.help(i18n::cli_open_url_help()))
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
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,chromiumoxide=off")),
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

    let token = CancellationToken::new();
    let token_clone = token.clone();

    // ctrl+d
    let stdin_eof = async {
        let mut buf = [0u8; 1];
        let _ = tokio::io::stdin().read(&mut buf).await;
    };

    let interrupt = async {
        tokio::select! {
            _ = stdin_eof => {},
            _ = tokio::signal::ctrl_c() => {},
        }
    };

    let result = tokio::select! {
        res = async { run_command(cli.command, &opts, &out, token_clone).await } => res,
        _ = interrupt => {
            token.cancel();
            eprintln!("\nInterrupted, shutting down...");
            tokio::time::sleep(Duration::from_secs(2)).await;
            Err(anyhow::anyhow!("interrupted"))
        }
    };

    if result.is_err() {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}

async fn run_command(
    command: Commands,
    opts: &BrowserOptions,
    out: &Output,
    token: CancellationToken,
) -> anyhow::Result<()> {
    match command {
        Commands::Auth { command } => match command {
            AuthCommands::Login => {
                let result = auth::login(opts, &token).await?;
                out.result(&result);
            }
            AuthCommands::Logout => {
                let result = auth::logout(opts).await?;
                out.result(&result);
            }
            AuthCommands::Status => {
                let result = auth::check_status(opts).await?;
                out.result(&result);
            }
        },
        Commands::Explore {
            keywords,
            exclude,
            max_notes,
            scroll_speed,
            interact,
            duration,
        } => {
            let speed: ScrollSpeed = scroll_speed.parse()?;
            let result = explore::run(
                &explore::ExploreOptions {
                    keywords,
                    exclude,
                    max_notes,
                    scroll_speed: speed,
                    interact,
                    duration,
                },
                opts,
                &token,
            )
            .await?;
            out.result(&result);
        }
        Commands::Search {
            query,
            sort_by,
            note_type,
            publish_time,
            search_scope,
            location,
            max_notes,
            scroll_speed,
            duration,
        } => {
            let speed: ScrollSpeed = scroll_speed.parse()?;
            let result = search::run(
                &search::SearchOptions {
                    query,
                    sort_by: sort_by.map(|s| s.parse()).transpose()?,
                    note_type: note_type.map(|s| s.parse()).transpose()?,
                    publish_time: publish_time.map(|s| s.parse()).transpose()?,
                    search_scope: search_scope.map(|s| s.parse()).transpose()?,
                    location: location.map(|s| s.parse()).transpose()?,
                    max_notes,
                    scroll_speed: speed,
                    duration,
                },
                opts,
                &token,
            )
            .await?;
            out.result(&result);
        }
        Commands::Creator {
            url,
            max_notes,
            scroll_speed,
            duration,
        } => {
            let speed: ScrollSpeed = scroll_speed.parse()?;
            let result = creator::run(
                &creator::CreatorOptions {
                    url,
                    max_notes,
                    scroll_speed: speed,
                    duration,
                },
                opts,
                &token,
            )
            .await?;
            out.result(&result);
        }
        Commands::Note {
            url,
            max_comments,
            max_replies,
            scroll_speed,
            duration,
        } => {
            let speed: ScrollSpeed = scroll_speed.parse()?;
            let result = note::run(
                &note::NoteOptions {
                    url,
                    max_comments,
                    max_replies,
                    scroll_speed: speed,
                    duration,
                },
                opts,
            )
            .await?;
            out.result(&result);
        }
        Commands::Open { url } => {
            let mut browser = create_browser(opts).await?;
            let page = create_page_with_cookies(&browser, &url).await?;
            println!("{}", i18n::cli_open_about());

            let browser_closed = xiaohongshu_tools::shared::utils::wait_for_page_close(
                &page,
                std::time::Duration::from_secs(2),
            );

            let cancelled = token.cancelled();

            tokio::select! {
                _ = browser_closed => {}
                _ = cancelled => {}
            }

            browser.close().await?;
        }
    }

    Ok(())
}
