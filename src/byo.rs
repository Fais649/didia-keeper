//! Clia's "bring your own Claude".
//!
//! Dia runs its AI agent with a bundled copy of Claude Code
//! (`Contents/Resources/agent-server-resources/dist/claude`), started by Dia's agent
//! server under the sandbox profile `agent-claude-code.sb`. The server points it at Dia's
//! own model gateway on localhost (Vertex mode: CLAUDE_CODE_USE_VERTEX=1 plus a random
//! ANTHROPIC_AUTH_TOKEN). Clia:
//!   1. renames that program to `claude-dia` and puts a small launcher named `claude` in
//!      its place, which drops the gateway settings, hands Claude Code the person's own
//!      subscription token (from `claude setup-token`) and runs the real program;
//!   2. lets the sandbox start `claude-dia`, read the token file, and reach the
//!      Anthropic API over HTTPS. Dia's tools still run through its localhost gateway.
use crate::keeper::work_dir;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn token_path() -> PathBuf {
    work_dir().join("claude-token")
}

pub fn has_token() -> bool {
    fs::read_to_string(token_path()).map(|t| looks_like_token(t.trim())).unwrap_or(false)
}

pub fn looks_like_token(t: &str) -> bool {
    t.starts_with("sk-ant-") && t.len() > 40 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Find a subscription token in text (e.g. the output of `claude setup-token`).
/// A long token can be wrapped onto following lines; those are joined back on.
pub fn find_token(text: &str) -> Option<String> {
    let clean = strip_ansi(text);
    let is_tok = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    let lines: Vec<&str> = clean.split(['\n', '\r']).map(str::trim).filter(|l| !l.is_empty()).collect();
    for (i, line) in lines.iter().enumerate() {
        if let Some(start) = line.find("sk-ant-oat") {
            let mut tok: String = line[start..].chars().take_while(|&c| is_tok(c)).collect();
            if tok.len() == line.len() - start {
                for next in &lines[i + 1..] {
                    if next.chars().all(is_tok) {
                        tok.push_str(next);
                    } else {
                        break;
                    }
                }
            }
            if looks_like_token(&tok) {
                return Some(tok);
            }
        }
    }
    None
}

fn strip_ansi(s: &str) -> String {
    regex::Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07").unwrap().replace_all(s, "").into_owned()
}

pub fn save_token(token: &str) -> Result<(), String> {
    let t = token.trim();
    if !looks_like_token(t) {
        return Err("That doesn't look like a Claude token (it starts with sk-ant-).".into());
    }
    fs::create_dir_all(work_dir()).map_err(|e| e.to_string())?;
    fs::write(token_path(), t).map_err(|e| e.to_string())?;
    fs::set_permissions(token_path(), fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    crate::keeper::log("saved your Claude subscription token for Clia");
    Ok(())
}

pub fn clear_token() {
    let _ = fs::remove_file(token_path());
    crate::keeper::log("removed the Claude subscription token");
}

/// The person's own Claude Code, to run `claude setup-token` with.
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

/// Where a `claude setup-token` session started from the window is recorded.
pub fn setup_transcript() -> PathBuf {
    work_dir().join("setup-token.txt")
}

/// Open Terminal with `claude setup-token` (recorded, so the token can be picked up).
pub fn start_setup_token() -> Result<(), String> {
    let claude = local_claude().ok_or("Claude Code isn't installed. Install it from claude.com/code, then try again.")?;
    fs::create_dir_all(work_dir()).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(setup_transcript());
    let q = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "'\\''"));
    let cmd = format!(
        "clear; echo 'Sign in with your Claude subscription. Clia picks up the token by itself; you can close this window afterwards.'; /usr/bin/script -q {} {} setup-token",
        q(&setup_transcript()),
        q(&claude)
    );
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
        cmd.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let out = std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("couldn't open Terminal: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

/// Pick up a token from a running or finished setup session. Removes the recording once read.
pub fn collect_setup_token() -> Option<String> {
    let text = fs::read(setup_transcript()).ok()?;
    let t = find_token(&String::from_utf8_lossy(&text))?;
    if save_token(&t).is_ok() {
        let _ = fs::remove_file(setup_transcript());
        Some(t)
    } else {
        None
    }
}

const LAUNCHER: &str = r#"#!/bin/sh
# Clia: run Dia's agent on your own Claude subscription (written by Didia Keeper).
# Uses only shell built-ins: the agent sandbox allows few other programs.
DIR=${0%/*}
TOKEN_FILE='__TOKEN_FILE__'
if [ -r "$TOKEN_FILE" ]; then
  read -r CLAUDE_CODE_OAUTH_TOKEN < "$TOKEN_FILE"
  export CLAUDE_CODE_OAUTH_TOKEN
  unset CLAUDE_CODE_USE_VERTEX CLAUDE_CODE_SKIP_VERTEX_AUTH ANTHROPIC_VERTEX_PROJECT_ID \
        ANTHROPIC_VERTEX_BASE_URL CLOUD_ML_REGION ANTHROPIC_AUTH_TOKEN ANTHROPIC_API_KEY \
        ANTHROPIC_BASE_URL ANTHROPIC_DEFAULT_OPUS_MODEL ANTHROPIC_DEFAULT_SONNET_MODEL \
        ANTHROPIC_DEFAULT_HAIKU_MODEL ANTHROPIC_SMALL_FAST_MODEL ANTHROPIC_MODEL
  # Dia names models the Google Cloud way (name@date); Anthropic's API wants name-date.
  n=$#
  while [ "$n" -gt 0 ]; do
    a=$1; shift
    case "$a" in claude-*@*) a="${a%%@*}-${a#*@}" ;; esac
    set -- "$@" "$a"
    n=$((n - 1))
  done
fi
exec "$DIR/claude-dia" "$@"
"#;

fn sandbox_additions(token: &Path) -> String {
    format!(
        r#"

;; ============================================================================
;; Clia (Didia Keeper): Claude Code on the person's own Claude subscription
;; ============================================================================
(allow process-exec (literal (string-append (param "BIN_DIR") "/claude-dia")))
(allow file-read* (literal "{}"))
(allow network-outbound (remote tcp "*:443"))
(allow network-outbound (literal "/private/var/run/mDNSResponder"))
"#,
        token.display()
    )
}

/// Apply the launcher and sandbox changes to a copied Dia's Contents folder.
pub fn apply(contents: &Path) -> Result<(), String> {
    let dist = contents.join("Resources/agent-server-resources/dist");
    let claude = dist.join("claude");
    let real = dist.join("claude-dia");
    let sb = dist.join("agent-claude-code.sb");
    if !claude.is_file() || !sb.is_file() {
        return Err("this Dia release has no built-in Claude agent, so Clia can't use your own Claude with it".into());
    }
    if !real.exists() {
        fs::rename(&claude, &real).map_err(|e| e.to_string())?;
    }
    let launcher = LAUNCHER.replace("__TOKEN_FILE__", &token_path().display().to_string().replace('\'', ""));
    fs::write(&claude, launcher).map_err(|e| e.to_string())?;
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    let mut profile = fs::read_to_string(&sb).map_err(|e| e.to_string())?;
    if !profile.contains("Clia (Didia Keeper)") {
        profile.push_str(&sandbox_additions(&token_path()));
        fs::write(&sb, profile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_wrapped_token() {
        let t = "Your token:\r\n\x1b[1msk-ant-oat01-AbCdEfGhIjKlMn\r\nOpQrStUvWxYz0123456789_-abc\x1b[0m\r\n\r\nStore it";
        assert_eq!(super::find_token(t).unwrap(), "sk-ant-oat01-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-abc");
    }
}
