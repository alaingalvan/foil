use chrono::Utc;
use std::process::Command;

fn run_command(name: &str, args: &[&str]) -> String {
    let output = match Command::new(name).args(args).output() {
        Ok(output) => output,
        Err(_) => panic!("Failed to run '{name:?} {:?}'. Is '{name}' installed and in your PATH?", name),
    };

    if !output.status.success() {
        panic!(
            "'{name} {:?}' command failed with status {}",
            args,
            output.status
        );
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string()
}

fn main() {
    // ⌚ Build time:
    let cur_time = Utc::now();
    let cur_time_str = cur_time.format("%Y-%m-%dT%H:%MZ");
    println!("cargo::rustc-env=BUILD_TIME={}", cur_time_str);

    // 🌳 Current branch:
    let cur_branch = run_command("git", &["rev-parse", "--abbrev-ref", "HEAD"]);
    println!("cargo::rustc-env=BUILD_GIT_BRANCH={}", cur_branch);

    // 🍃 Current commit:
    let cur_commit = run_command("git", &["rev-parse", "--short", "HEAD"]);
    println!("cargo::rustc-env=BUILD_GIT_COMMIT={}", cur_commit);
}
