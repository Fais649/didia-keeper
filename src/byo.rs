//! Clia's AI: the person's own Claude Code (and so their Claude subscription).
//!
//! Dia's built-in AI is tied to Dia's own plans, so Clia switches it off and instead adds
//! a "Claude" panel (⇧⌘K, in patch/didia_patch.m) that runs the Claude Code installed on
//! this Mac. The panel reads the active tab through Dia's AppleScript support and sends
//! the question to `claude -p`, so answers use whatever account Claude Code is signed in to.
use std::path::{Path, PathBuf};

/// The person's own Claude Code.
pub fn local_claude() -> Option<PathBuf> {
    let home = crate::keeper::home();
    let mut c = vec![
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];
    if let Ok(path) = std::env::var("PATH") {
        c.extend(path.split(':').map(|d| Path::new(d).join("claude")));
    }
    c.into_iter().find(|p| p.is_file())
}

/// "2.1.325 (Claude Code)", when Claude Code is installed.
pub fn claude_version() -> Option<String> {
    let path = local_claude()?;
    let out = std::process::Command::new(path).arg("--version").output().ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty()).then_some(v)
}
