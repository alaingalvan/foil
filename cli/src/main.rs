#![warn(unused_extern_crates)]
#![warn(unused_crate_dependencies)]

mod builder;
mod error;
mod misc;
mod query_post;
mod reset;
mod server;

use builder::{build, BuildMode};
use chrono::Utc;
use clap::{arg, ArgMatches, Command};
use lazy_static::lazy_static;
use reset::reset;
use server::start_server;
use std::io::{stdout, Write};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn get_build_mode(default: BuildMode, sub_m: &ArgMatches) -> BuildMode {
    if let Some(v) = sub_m.get_one::<bool>("release") {
        if *v {
            return BuildMode::Release;
        }
    }
    if let Some(v) = sub_m.get_one::<bool>("dev") {
        if *v {
            return BuildMode::Development;
        }
    }
    default
}

lazy_static! {
    static ref BUILD_DATE: String = Utc::now().format("%m/%d/%Y").to_string();
}

#[async_std::main]
async fn main() -> Result<()> {
    println!("✨ Foil CLI (v{})", env!("CARGO_PKG_VERSION"));
    if cfg!(dev) {
        println!(
            "🌃 Build {} | {} | {}",
            env!("BUILD_GIT_BRANCH"),
            env!("BUILD_GIT_COMMIT"),
            env!("BUILD_TIME")
        );
    }
    let mut app = Command::new("✨ foil")
        .version("0.1.0")
        .about("💫 Foil's primary CLI application, provides everything needed to start and manage a foil project.")
        .subcommand(
            Command::new("build")
                .display_order(3)
                .about("🛠️ Build your foil project, both the frontend/portfolio.")
                .args(&[arg!(--release "🧑‍💼 Build your frontend and backend in Release mode (default)."),
                        arg!(--dev "🧑‍💻 Build your frontend and backend in Development mode."),
                        arg!(--watch "👁️ Build your foil project and automatically compile any changes to it.")])
        )
        .subcommand(
            Command::new("server")
                .display_order(4)
                .about("🖥️ Manage the Foil server.")
                .subcommand(
                    Command::new("start")
                        .about("Start the foil server.")
                        .arg(arg!(--release "🧑‍💼 Runs server in Release mode. (default)"))
                        .arg(arg!(--dev "🧑‍💻 Runs server in Development mode.")))
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
        Some(("build", sub_m)) => {
            let build_mode = get_build_mode(BuildMode::Release, sub_m);
            let _ = build(build_mode.clone()).await;
        }
        Some(("server", sub_m)) => {
            match sub_m.subcommand() {
                Some(("start", sub_m)) => {
                    let build_mode = get_build_mode(BuildMode::Release, sub_m);
                    let _ = start_server(build_mode.clone()).await;
                }
                Some(("reset", _sub_m)) => {
                    let _ = reset().await;
                }
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
