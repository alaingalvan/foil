use std::env;
use std::process::Command;
use std::thread;

use super::APIRequest;

pub async fn update(data: APIRequest) {
    // Spawn thread to update local repo
    if data.refs != "refs/heads/master" {
        return;
    } else {
        // Depending on what's changed, update each package accordingly.
        thread::spawn(move || {
            Command::new("git")
                .arg("pull")
                .output()
                .expect("Failed to pull from git!");

            Command::new("npm")
                .arg("--prefix")
                .current_dir(
                    env::current_dir()
                        .unwrap()
                        .join("../../")
                        .canonicalize()
                        .unwrap(),
                )
                .arg("ci")
                .arg("--include=dev")
                .output()
                .expect("Failed to run NPM CI!");

            Command::new("foil-cli")
                .arg("build")
                .output()
                .expect("Failed to build from foil CLI!");
        });
    }
}
