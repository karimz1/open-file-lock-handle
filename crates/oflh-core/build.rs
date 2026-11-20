//! Record the git commit for local builds when CI has not already supplied one.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-env-changed=OFLH_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=OFLH_SOURCE_COMMIT");

    let commit = env_commit("OFLH_SOURCE_COMMIT").unwrap_or_else(discover_commit);
    // rustc-env values must be a single line.
    let commit = commit.lines().next().unwrap_or("").trim().to_string();
    println!("cargo:rustc-env=OFLH_SOURCE_COMMIT={commit}");
}

fn env_commit(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn discover_commit() -> String {
    let Some(root) = repo_root() else {
        return String::new();
    };
    watch_git(&root);
    let output = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "rev-parse",
            "--short",
            "HEAD",
        ])
        .output();
    let Ok(output) = output else {
        return String::new();
    };
    if !output.status.success() {
        return String::new();
    }
    let mut commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if commit.is_empty()
        || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !(7..=40).contains(&commit.len())
    {
        return String::new();
    }
    if is_dirty(&root) {
        commit.push_str("-dirty");
    }
    commit
}

fn repo_root() -> Option<PathBuf> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    for dir in manifest.ancestors() {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn watch_git(root: &Path) {
    let git = root.join(".git");
    println!("cargo:rerun-if-changed={}", git.display());
    if git.is_file() {
        if let Ok(text) = fs::read_to_string(&git)
            && let Some(dir) = text
                .strip_prefix("gitdir:")
                .or_else(|| text.lines().find_map(|line| line.strip_prefix("gitdir:")))
                .map(str::trim)
        {
            let git_dir = if Path::new(dir).is_absolute() {
                PathBuf::from(dir)
            } else {
                root.join(dir)
            };
            watch_git_dir(&git_dir);
        }
        return;
    }
    if git.is_dir() {
        watch_git_dir(&git);
    }
}

fn watch_git_dir(git_dir: &Path) {
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    if let Ok(text) = fs::read_to_string(&head)
        && let Some(ref_path) = text.strip_prefix("ref:").map(str::trim)
    {
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join(ref_path).display()
        );
    }
    let index = git_dir.join("index");
    if index.exists() {
        println!("cargo:rerun-if-changed={}", index.display());
    }
}

fn is_dirty(root: &Path) -> bool {
    // `git status --porcelain` lists modified and untracked files, but not
    // ignored paths such as target/ or node_modules/. Those must not mark a
    // local build dirty.
    Command::new("git")
        .args(["-C", &root.to_string_lossy(), "status", "--porcelain"])
        .output()
        .ok()
        .is_some_and(|output| output.status.success() && !output.stdout.is_empty())
}
