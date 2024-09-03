use chrono::Utc;
use std::process::Command;

fn main() {
    // ⌚ Build time:
    let cur_time = Utc::now();
    let cur_time_str = cur_time.format("%Y-%m-%dT%H:%MZ");
    println!("cargo::rustc-env=BUILD_TIME={}", cur_time_str);

    // 🌳 Current branch:
    let cur_branch = Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .unwrap()
        .stdout;
    let cur_branch_str = String::from_utf8_lossy(&cur_branch).trim().to_string();
    println!("cargo::rustc-env=BUILD_GIT_BRANCH={}", cur_branch_str);

    // 🍃 Current commit:
    let cur_commit = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .unwrap()
        .stdout;
    let cur_commit_str = String::from_utf8_lossy(&cur_commit).trim().to_string();
    println!("cargo::rustc-env=BUILD_GIT_COMMIT={}", cur_commit_str);
}
