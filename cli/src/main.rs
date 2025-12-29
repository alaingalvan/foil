#![warn(unused_extern_crates)]
#![warn(unused_crate_dependencies)]

mod builder;
mod error;
mod misc;
mod query_post;
mod reset;
mod server;

use builder::{BuildMode, build};
use chrono::Utc;

use clap::{ArgMatches, Command, arg};
use lazy_static::lazy_static;
use reset::reset;
use server::start_server;
use std::io::{Write, stdout};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn get_build_mode(sub_match: &ArgMatches) -> BuildMode {
    if let Some(v) = sub_match.get_one::<bool>("release")
        && *v
    {
        BuildMode::Release
    } else if let Some(v) = sub_match.get_one::<bool>("dev")
        && *v
    {
        BuildMode::Development
    } else {
        BuildMode::Release
    }
}

lazy_static! {
    static ref BUILD_DATE: String = Utc::now().format("%m/%d/%Y").to_string();
    static ref APP_VERSION: String = format!("v{}", env!("CARGO_PKG_VERSION"));
    static ref APP_VERSION_LONG: String = format!(
        "{}\n🌃 Build {} | {} | {}",
        APP_VERSION.as_str(),
        env!("BUILD_GIT_BRANCH"),
        env!("BUILD_GIT_COMMIT"),
        BUILD_DATE.as_str()
    );
}

#[async_std::main]
async fn main() -> Result<()> {
    let mut app = Command::new("✨ foil")
        .version(APP_VERSION.as_str())
        .long_version(APP_VERSION_LONG.as_str())
        .about("💫 Foil's primary CLI application, provides everything needed to start and manage a foil project.")
        .subcommand(
            Command::new("build")
                .display_order(3)
                .about("🛠️ Build your foil project, both the frontend/portfolio.")
                .args(&[arg!(--release "💼 Build your frontend and backend in Release mode (default)."),
                        arg!(--dev "🪲 Build your frontend and backend in Development mode."),
                        arg!(--watch "👁️ Build your foil project and automatically compile any changes to it.")])
        )
        .subcommand(
            Command::new("server")
                .display_order(4)
                .about("🖥️ Manage the Foil server.")
                .subcommand(
                    Command::new("start")
                        .about("Start the foil server.")
                        .arg(arg!(--release "💼 Runs server in Release mode. (default)"))
                        .arg(arg!(--dev "🪲 Runs server in Development mode.")))
                .subcommand(
                    Command::new("reset")
                    .about("Reset the server database."))
        );

    // ❔ Write out long help if no args exist
    let mut out = stdout();
    let mut vec = Vec::with_capacity(1024);
    app.write_long_help(&mut vec)
        .expect("failed to write to stdout");

    let matches = app.get_matches();
    match matches.subcommand() {
        Some(("build", sub_match)) => {
            let build_mode = get_build_mode(sub_match);
            build(build_mode.clone()).await?;
        }
        Some(("server", sub_match)) => {
            match sub_match.subcommand() {
                Some(("start", sub_match)) => {
                    let build_mode = get_build_mode(sub_match);
                    if sub_match.get_one::<bool>("watch").copied().unwrap_or(false) {
                        out.write(b"Watch mode is currently not implemented.")?;
                    } else {
                        start_server(build_mode).await?;
                    }
                }
                Some(("reset", _sub_m)) => reset().await?,
                _ => (),
            };
        }
        _ => {
            out.write_all(&vec)
                .expect("Fail: Could not write to standard out.");
        }
    };
    Ok(())
}
