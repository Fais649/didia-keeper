//! Didia Keeper's work: read Dia's official update feed, download and verify official
//! releases, build Didia or Clia from them (patch + re-sign) and install it.
use crate::{flags, macho};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub const FEED_URL: &str = "https://releases.diabrowser.com/BoostBrowser-updates.xml";
/// The Browser Company's Apple team id; downloads signed by anyone else are refused.
pub const DIA_TEAM_ID: &str = "S6N382Y83G";
pub const CHECK_EVERY_SECS: u64 = 4 * 60 * 60;
pub const AGENT_INTERVAL_SECS: u64 = 600;
pub const AGENT_LABEL: &str = "com.didia.keeper";
const PATCH_NAME: &str = "@executable_path/../Frameworks/libdidia.dylib";
const PATCH_DYLIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libdidia.dylib"));
const ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>com.apple.security.device.audio-input</key><true/>
<key>com.apple.security.device.camera</key><true/>
<key>com.apple.security.personal-information.location</key><true/>
</dict></plist>
"#;

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}
pub fn work_dir() -> PathBuf {
    home().join("Library/Application Support/Didia Keeper")
}
pub fn log_path() -> PathBuf {
    home().join("Library/Logs/DidiaKeeper.log")
}
pub fn pristine_app() -> PathBuf {
    work_dir().join("pristine/Dia.app")
}
pub fn staged_app(e: Edition) -> PathBuf {
    work_dir().join(format!("staged/{}.app", e.name()))
}
pub fn agent_plist() -> PathBuf {
    home().join(format!("Library/LaunchAgents/{AGENT_LABEL}.plist"))
}

/// Which browser to make from official Dia.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    /// Dia without AI and without Atlassian.
    #[default]
    Didia,
    /// Dia's plan-locked AI and Atlassian removed; a Claude panel on your own Claude Code instead.
    Clia,
}
impl Edition {
    pub fn name(self) -> &'static str {
        match self {
            Edition::Didia => "Didia",
            Edition::Clia => "Clia",
        }
    }
    pub fn install_path(self) -> PathBuf {
        PathBuf::from(format!("/Applications/{}.app", self.name()))
    }
    pub fn other(self) -> Edition {
        match self {
            Edition::Didia => Edition::Clia,
            Edition::Clia => Edition::Didia,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub edition: Edition,
    /// Didia: switch Dia's AI off.
    pub remove_ai: bool,
    /// Didia: switch Atlassian off (Clia always does).
    pub remove_atlassian: bool,
    pub applescript_js: bool,
    /// Clia: add the Claude panel that runs the person's own Claude Code.
    pub own_claude: bool,
    /// Clia: make a daily brief from the person's connected apps each morning.
    pub daily_brief: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Settings { edition: Edition::Didia, remove_ai: true, remove_atlassian: true, applescript_js: false, own_claude: true, daily_brief: true }
    }
}
impl Settings {
    /// (AI off, Atlassian off, Dia's paid-plan prompts off) for the chosen edition.
    /// Clia switches Dia's own AI off too (it is tied to Dia's plans) and brings its own panel.
    pub fn switches(&self) -> (bool, bool, bool) {
        match self.edition {
            Edition::Didia => (self.remove_ai, self.remove_atlassian, self.remove_ai),
            Edition::Clia => (true, true, true),
        }
    }
    pub fn load() -> Self {
        fs::read(work_dir().join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let _ = fs::create_dir_all(work_dir());
        let _ = fs::write(work_dir().join("settings.json"), serde_json::to_vec_pretty(self).unwrap());
    }
}

pub fn log(msg: &str) {
    let line = format!("{} {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), msg);
    if let Some(p) = log_path().parent() {
        let _ = fs::create_dir_all(p);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn log_tail(lines: usize) -> Vec<String> {
    let text = fs::read_to_string(log_path()).unwrap_or_default();
    let all: Vec<String> = text.lines().map(str::to_string).collect();
    all[all.len().saturating_sub(lines)..].to_vec()
}

/// What an app bundle says about itself.
#[derive(Clone, Default)]
pub struct AppInfo {
    pub version: String,
    pub build: u64,
    pub made_by: MadeBy,
}
#[derive(Clone, Default, PartialEq)]
pub enum MadeBy {
    #[default]
    Unknown,
    Keeper { edition: Edition, ai_off: bool, atlassian_off: bool, own_claude: bool },
    Official,
}

pub fn app_info(app: &Path) -> Option<AppInfo> {
    let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let d = info.as_dictionary()?;
    let version = d.get("CFBundleShortVersionString")?.as_string()?.to_string();
    let build = d.get("CFBundleVersion").and_then(|v| v.as_string()).and_then(|s| s.parse().ok()).unwrap_or(0);
    let res = app.join("Contents/Resources/DidiaPatch.plist");
    let made_by = if let Ok(p) = plist::Value::from_file(&res) {
        let pd = p.as_dictionary().cloned().unwrap_or_default();
        let b = |k: &str| pd.get(k).and_then(|v| v.as_boolean()).unwrap_or(false);
        let edition = if pd.get("edition").and_then(|v| v.as_string()) == Some("Clia") { Edition::Clia } else { Edition::Didia };
        MadeBy::Keeper { edition, ai_off: b("removeAI"), atlassian_off: b("removeAtlassian"), own_claude: b("claudePanel") }
    } else if matches!(d.get("CFBundleDisplayName").and_then(|v| v.as_string()), Some("Didia") | Some("Clia")) {
        MadeBy::Unknown
    } else {
        MadeBy::Official
    };
    Some(AppInfo { version, build, made_by })
}

#[derive(Clone, Debug)]
pub struct Release {
    pub url: String,
    pub build: u64,
    pub version: String,
    pub length: u64,
}

/// Newest full (non-delta) release in the Sparkle feed.
pub fn parse_feed(xml: &str) -> Option<Release> {
    let enc = regex::Regex::new(r"<enclosure\s[^>]*>").unwrap();
    let attr = |tag: &str, name: &str| -> Option<String> {
        let re = regex::Regex::new(&format!(r#"\s{}="([^"]*)""#, regex::escape(name))).unwrap();
        re.captures(tag).map(|c| c[1].to_string())
    };
    enc.find_iter(xml)
        .filter_map(|m| {
            let tag = m.as_str();
            let url = attr(tag, "url")?;
            if !url.ends_with(".zip") || !url.starts_with("https://releases.diabrowser.com/") {
                return None;
            }
            let build = attr(tag, "sparkle:version")?.parse().ok()?;
            let version = attr(tag, "sparkle:shortVersionString")
                .map(|v| v.split(' ').next().unwrap_or("").to_string())
                .unwrap_or_default();
            let length = attr(tag, "length").and_then(|l| l.parse().ok()).unwrap_or(0);
            Some(Release { url, build, version, length })
        })
        .max_by_key(|r| r.build)
}

pub fn fetch_latest() -> Result<Release, String> {
    let out = Command::new("/usr/bin/curl")
        .args(["-fsS", "--max-time", "30", FEED_URL])
        .output()
        .map_err(|e| format!("couldn't run curl: {e}"))?;
    if !out.status.success() {
        return Err("couldn't reach Dia's update feed".into());
    }
    parse_feed(&String::from_utf8_lossy(&out.stdout)).ok_or_else(|| "couldn't read Dia's update feed".into())
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!("{} {} failed: {}", cmd, args.join(" "), text.trim()))
    }
}

/// Official Dia, signed by The Browser Company and accepted by Gatekeeper.
pub fn verify_official(app: &Path) -> Result<(), String> {
    let p = app.to_str().unwrap();
    run("/usr/bin/codesign", &["--verify", "--deep", "--strict", p])?;
    let info = run("/usr/bin/codesign", &["-dv", p])?;
    let team = info.lines().find_map(|l| l.strip_prefix("TeamIdentifier=")).unwrap_or("").trim().to_string();
    if team != DIA_TEAM_ID {
        return Err(format!("refused: signed by '{team}', not The Browser Company"));
    }
    run("/usr/sbin/spctl", &["-a", "-t", "exec", p]).map_err(|_| "refused: Gatekeeper doesn't accept it".to_string())?;
    Ok(())
}

pub fn is_running(e: Edition) -> bool {
    Command::new("/usr/bin/pgrep")
        .args(["-f", &format!("{}/Contents/MacOS/", e.install_path().display())])
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Quit an edition the normal way (Dia restores its windows next time), waiting up to 30 s.
pub fn quit_edition(e: Edition) -> Result<(), String> {
    if !is_running(e) {
        return Ok(());
    }
    let script = format!("tell application \"{}\" to quit", e.install_path().display());
    let _ = Command::new("/usr/bin/osascript").args(["-e", &script]).output();
    for _ in 0..60 {
        if !is_running(e) {
            log(&format!("quit {}", e.name()));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!("{} didn't quit; quit it yourself, then open {}", e.name(), e.other().name()))
}

/// Open an edition. Didia and Clia share Dia's profile, so the other one is quit first
/// (otherwise the running one would just take over the request).
pub fn open_edition(e: Edition) -> Result<String, String> {
    if !e.install_path().exists() {
        return Err(format!("{} isn't installed yet", e.name()));
    }
    quit_edition(e.other())?;
    run("/usr/bin/open", &[e.install_path().to_str().unwrap()])?;
    log(&format!("opened {}", e.name()));
    Ok(format!("{} is open.", e.name()))
}

/// Whether the installed edition matches the current choices (else it needs a rebuild).
pub fn installed_matches(s: &Settings) -> bool {
    let (ai, atl, _) = s.switches();
    let own = s.edition == Edition::Clia && s.own_claude;
    matches!(
        app_info(&s.edition.install_path()).map(|i| i.made_by),
        Some(MadeBy::Keeper { ai_off, atlassian_off, own_claude, .. }) if ai_off == ai && atlassian_off == atl && own_claude == own
    )
}

fn notify(msg: &str) {
    let script = format!("display notification \"{}\" with title \"Didia\"", msg.replace('"', "'"));
    let _ = Command::new("/usr/bin/osascript").args(["-e", &script]).output();
}

/// Progress shared with the window.
#[derive(Default, Clone)]
pub struct Progress {
    pub busy: bool,
    pub step: String,
    pub fraction: Option<f32>,
    pub latest: Option<Release>,
    pub error: Option<String>,
    pub last_result: Option<String>,
}
pub type Shared = Arc<Mutex<Progress>>;

fn set_step(p: &Option<Shared>, step: &str, fraction: Option<f32>) {
    if let Some(p) = p {
        let mut g = p.lock().unwrap();
        g.step = step.to_string();
        g.fraction = fraction;
    }
}

/// Download, unpack and verify an official release; it becomes the new pristine copy.
pub fn download_release(rel: &Release, p: &Option<Shared>) -> Result<(), String> {
    log(&format!("downloading official Dia {} ({}): {}", rel.version, rel.build, rel.url));
    let tmp = work_dir().join("download");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let zip = tmp.join("dia.zip");
    let mut child = Command::new("/usr/bin/curl")
        .args(["-fsSL", "--max-time", "1800", "-o"])
        .arg(&zip)
        .arg(&rel.url)
        .spawn()
        .map_err(|e| e.to_string())?;
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            if !st.success() {
                return Err("the download failed".into());
            }
            break;
        }
        let got = fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
        let frac = if rel.length > 0 { Some((got as f32 / rel.length as f32).min(1.0)) } else { None };
        set_step(p, &format!("Downloading Dia {} — {} of {} MB", rel.version, got / 1_000_000, rel.length / 1_000_000), frac);
        std::thread::sleep(Duration::from_millis(300));
    }
    set_step(p, "Unpacking", None);
    let x = tmp.join("x");
    run("/usr/bin/ditto", &["-x", "-k", zip.to_str().unwrap(), x.to_str().unwrap()])?;
    let app = fs::read_dir(&x)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == "app").unwrap_or(false))
        .ok_or("no app in the download")?;
    set_step(p, "Checking Apple's signature", None);
    verify_official(&app)?;
    let _ = fs::remove_dir_all(pristine_app());
    fs::create_dir_all(pristine_app().parent().unwrap()).map_err(|e| e.to_string())?;
    fs::rename(&app, pristine_app()).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&tmp);
    log(&format!("verified official Dia {} ({}), signed by {DIA_TEAM_ID}", rel.version, rel.build));
    Ok(())
}

/// Official Dia apps already on this Mac (never changed; only copied from).
pub fn official_dia_on_mac() -> Option<(PathBuf, AppInfo)> {
    [PathBuf::from("/Applications/Dia.app"), home().join("Applications/Dia.app")]
        .into_iter()
        .filter_map(|c| app_info(&c).map(|i| (c, i)))
        .filter(|(_, i)| i.made_by == MadeBy::Official)
        .max_by_key(|(_, i)| i.build)
}

/// Make sure there's a verified official copy to build from. An official Dia already on
/// this Mac is used when it's newer than the kept copy (it is copied, never changed).
pub fn ensure_pristine(p: &Option<Shared>) -> Result<(), String> {
    let kept = app_info(&pristine_app()).map(|i| i.build).unwrap_or(0);
    if let Some((c, info)) = official_dia_on_mac() {
        if info.build > kept && verify_official(&c).is_ok() {
            set_step(p, "Copying the official Dia already on this Mac", None);
            let _ = fs::remove_dir_all(pristine_app());
            fs::create_dir_all(pristine_app().parent().unwrap()).map_err(|e| e.to_string())?;
            run("/usr/bin/ditto", &[c.to_str().unwrap(), pristine_app().to_str().unwrap()])?;
            log(&format!("using the official Dia {} at {}", info.version, c.display()));
            return Ok(());
        }
    }
    if kept > 0 {
        return Ok(());
    }
    set_step(p, "Reading Dia's update feed", None);
    let rel = fetch_latest()?;
    download_release(&rel, p)
}

/// Build Didia from a pristine official Dia.
pub fn build_edition(src: &Path, dest: &Path, s: &Settings, p: &Option<Shared>) -> Result<(), String> {
    let name = s.edition.name();
    set_step(p, "Copying Dia", None);
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
    run("/usr/bin/ditto", &[src.to_str().unwrap(), dest.to_str().unwrap()])?;
    let contents = dest.join("Contents");

    set_step(p, &format!("Adding the {name} patch"), None);
    fs::write(contents.join("Frameworks/libdidia.dylib"), PATCH_DYLIB).map_err(|e| e.to_string())?;
    let info_path = contents.join("Info.plist");
    let mut info = plist::Value::from_file(&info_path).map_err(|e| e.to_string())?;
    let exe_name = info
        .as_dictionary()
        .and_then(|d| d.get("CFBundleExecutable"))
        .and_then(|v| v.as_string())
        .unwrap_or("Dia")
        .to_string();
    let exe = contents.join("MacOS").join(&exe_name);
    let exe_s = exe.to_str().unwrap();
    if run("/usr/bin/lipo", &["-archs", exe_s]).map(|a| a.split_whitespace().count() > 1).unwrap_or(false) {
        run("/usr/bin/lipo", &["-thin", "arm64", exe_s, "-output", exe_s])?;
    }
    run("/usr/bin/codesign", &["--remove-signature", exe_s])?;
    let mut bytes = fs::read(&exe).map_err(|e| e.to_string())?;
    macho::add_load_dylib(&mut bytes, PATCH_NAME)?;
    fs::write(&exe, &bytes).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(contents.join("embedded.provisionprofile"));

    // Dia's own updater stays off: it would replace Didia with plain Dia.
    if let Some(d) = info.as_dictionary_mut() {
        d.insert("SUEnableAutomaticChecks".into(), plist::Value::Boolean(false));
        d.insert("SUAutomaticallyUpdate".into(), plist::Value::Boolean(false));
        d.remove("SUFeedURL");
        d.insert("CFBundleDisplayName".into(), plist::Value::String(name.into()));
    }
    info.to_file_xml(&info_path).map_err(|e| e.to_string())?;
    flags::patch_config(s)
        .to_file_xml(contents.join("Resources/DidiaPatch.plist"))
        .map_err(|e| e.to_string())?;

    set_step(p, &format!("Signing {name} for this Mac"), None);
    let ent = work_dir().join("entitlements.plist");
    fs::create_dir_all(work_dir()).map_err(|e| e.to_string())?;
    fs::write(&ent, ENTITLEMENTS).map_err(|e| e.to_string())?;
    let d = dest.to_str().unwrap();
    run("/usr/bin/codesign", &["--force", "--deep", "--sign", "-", d])?;
    run("/usr/bin/codesign", &["--force", "--sign", "-", "--entitlements", ent.to_str().unwrap(), d])?;
    run("/usr/bin/codesign", &["--verify", "--deep", "--strict", d])?;
    let _ = run("/usr/bin/xattr", &["-dr", "com.apple.quarantine", d]);
    let v = app_info(dest).map(|i| i.version).unwrap_or_default();
    let (ai, atl, _) = s.switches();
    log(&format!("built {name} {v} (AI off: {ai}, Atlassian off: {atl}, Claude panel: {})", s.edition == Edition::Clia && s.own_claude));
    Ok(())
}

/// Swap the staged build into /Applications (keeping the previous one aside).
pub fn install_staged(e: Edition) -> Result<String, String> {
    let name = e.name();
    let staged = staged_app(e);
    if !staged.exists() {
        return Err("nothing waiting to be installed".into());
    }
    if is_running(e) {
        return Err(format!("{name} is open; it will be installed after you quit it"));
    }
    let installed = e.install_path();
    let inst = installed.to_str().unwrap().to_string();
    let previous = work_dir().join(format!("previous/{name}.app"));
    let _ = fs::remove_dir_all(&previous);
    if installed.exists() {
        fs::create_dir_all(previous.parent().unwrap()).map_err(|e| e.to_string())?;
        // Same disk: a rename. Otherwise copy then delete.
        if fs::rename(&installed, &previous).is_err() {
            run("/usr/bin/ditto", &[&inst, previous.to_str().unwrap()])?;
            fs::remove_dir_all(&installed).map_err(|e| e.to_string())?;
        }
    }
    if fs::rename(&staged, &installed).is_err() {
        run("/usr/bin/ditto", &[staged.to_str().unwrap(), &inst])?;
        let _ = fs::remove_dir_all(&staged);
    }
    let _ = fs::remove_dir(staged.parent().unwrap());
    let _ = run(
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
        &["-f", &inst],
    );
    let v = app_info(&installed).map(|i| i.version).unwrap_or_default();
    log(&format!("installed {name} {v}"));
    Ok(v)
}

/// Build from the pristine copy, then install now or after the app quits.
pub fn rebuild_and_install(s: &Settings, p: &Option<Shared>) -> Result<String, String> {
    ensure_pristine(p)?;
    let e = s.edition;
    let name = e.name();
    build_edition(&pristine_app(), &staged_app(e), s, p)?;
    set_step(p, "Installing", None);
    if is_running(e) {
        let v = app_info(&staged_app(e)).map(|i| i.version).unwrap_or_default();
        log(&format!("{name} {v} is ready; it installs after {name} quits"));
        Ok(format!("{name} {v} is ready. Quit {name} and it installs on the next check (or press Install now)."))
    } else {
        install_staged(e).map(|v| format!("{name} {v} is installed."))
    }
}

/// Editions to keep current: the chosen one, plus any other one this tool installed.
pub fn editions_to_keep(s: &Settings) -> Vec<Edition> {
    let mut v = vec![s.edition];
    let o = s.edition.other();
    if matches!(app_info(&o.install_path()).map(|i| i.made_by), Some(MadeBy::Keeper { .. })) {
        v.push(o);
    }
    v
}

/// The background job: install waiting builds, then (at most every 4 hours) check the
/// official feed and rebuild every installed edition when a newer release is out.
pub fn check_and_update(force: bool, s: &Settings, p: &Option<Shared>) -> Result<String, String> {
    let editions = editions_to_keep(s);
    for &e in &editions {
        if staged_app(e).exists() && !is_running(e) {
            if let Ok(v) = install_staged(e) {
                notify(&format!("{} updated to {v}", e.name()));
            }
        }
    }
    let stamp = work_dir().join("last_check");
    let fresh = fs::metadata(&stamp)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age.as_secs() < CHECK_EVERY_SECS)
        .unwrap_or(false);
    if fresh && !force {
        return Ok("Checked recently.".into());
    }
    let _ = fs::create_dir_all(work_dir());
    let _ = fs::write(&stamp, b"");
    set_step(p, "Reading Dia's update feed", None);
    let rel = fetch_latest().map_err(|e| {
        log(&e);
        e
    })?;
    if let Some(p) = p {
        p.lock().unwrap().latest = Some(rel.clone());
    }
    let mut done = vec![];
    for &e in &editions {
        let se = Settings { edition: e, ..s.clone() };
        let have = app_info(&staged_app(e)).map(|i| i.build).unwrap_or(0);
        let installed_build = app_info(&e.install_path()).map(|i| i.build).unwrap_or(0);
        // The chosen edition is also rebuilt when its choices changed; others only for new releases.
        let current = rel.build <= have.max(installed_build) && (e != s.edition || installed_matches(&se));
        if current {
            continue;
        }
        if rel.build > app_info(&pristine_app()).map(|i| i.build).unwrap_or(0) {
            download_release(&rel, p)?;
        }
        done.push(rebuild_and_install(&se, p)?);
    }
    if done.is_empty() {
        let names: Vec<&str> = editions.iter().map(|e| e.name()).collect();
        return Ok(format!("{} up to date (Dia {}).", names.join(" and "), rel.version));
    }
    let msg = done.join(" ");
    if p.is_none() {
        notify(&msg);
    }
    Ok(msg)
}

/// A simple lock so the window and the background check never build at the same time.
pub struct Lock(PathBuf);
impl Lock {
    pub fn take() -> Option<Lock> {
        let path = work_dir().join("busy.lock");
        let _ = fs::create_dir_all(work_dir());
        if let Ok(m) = fs::metadata(&path) {
            let stale = m.modified().ok().and_then(|t| SystemTime::now().duration_since(t).ok())
                .map(|a| a.as_secs() > 2 * 60 * 60).unwrap_or(true);
            if !stale {
                return None;
            }
            let _ = fs::remove_dir(&path);
        }
        fs::create_dir(&path).ok().map(|_| Lock(path))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

// ---- Background updates (a LaunchAgent that runs this program with --background) ----

pub fn agent_installed() -> bool {
    agent_plist().exists()
}

fn uid() -> String {
    run("/usr/bin/id", &["-u"]).map(|s| s.trim().to_string()).unwrap_or_else(|_| "501".into())
}

pub fn enable_background_updates() -> Result<(), String> {
    // A stable copy of this program for the agent to run.
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin = work_dir().join("didia-keeper");
    fs::create_dir_all(work_dir()).map_err(|e| e.to_string())?;
    if me != bin {
        let _ = fs::remove_file(&bin);
        fs::copy(&me, &bin).map_err(|e| e.to_string())?;
    }
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>{AGENT_LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string><string>--background</string></array>
  <key>StartInterval</key><integer>{AGENT_INTERVAL_SECS}</integer>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>StandardErrorPath</key><string>{}</string>
</dict></plist>
"#,
        bin.display(),
        log_path().display()
    );
    fs::create_dir_all(agent_plist().parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(agent_plist(), plist).map_err(|e| e.to_string())?;
    let domain = format!("gui/{}", uid());
    let _ = run("/bin/launchctl", &["bootout", &format!("{domain}/{AGENT_LABEL}")]);
    run("/bin/launchctl", &["bootstrap", &domain, agent_plist().to_str().unwrap()])?;
    log("background updates on");
    Ok(())
}

/// Keep the background updater's copy of this program current (it runs that copy).
pub fn refresh_agent_copy() {
    let bin = work_dir().join("didia-keeper");
    let Ok(me) = std::env::current_exe() else { return };
    if !agent_installed() || me == bin {
        return;
    }
    let same = match (fs::read(&me), fs::read(&bin)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        let tmp = work_dir().join("didia-keeper.new");
        if fs::copy(&me, &tmp).is_ok() && fs::rename(&tmp, &bin).is_ok() {
            log("updated the background updater's copy of Didia Keeper");
        }
    }
}

pub fn disable_background_updates() -> Result<(), String> {
    let domain = format!("gui/{}", uid());
    let _ = run("/bin/launchctl", &["bootout", &format!("{domain}/{AGENT_LABEL}")]);
    let _ = fs::remove_file(agent_plist());
    log("background updates off");
    Ok(())
}

pub fn last_check() -> Option<SystemTime> {
    fs::metadata(work_dir().join("last_check")).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn feed_picks_newest_full_release() {
        let xml = r#"<item><enclosure url="https://releases.diabrowser.com/release/Dia-1.57.1-90935.zip" sparkle:version="90935" sparkle:shortVersionString="1.57.1 (90935)" length="732995488" />
<enclosure url="https://releases.diabrowser.com/release/Dia-from-90792-to-90935.delta" sparkle:deltaFrom="90792" length="1"/></item>
<item><enclosure url="https://releases.diabrowser.com/release/Dia-1.56.2-90792.zip" sparkle:version="90792" sparkle:shortVersionString="1.56.2 (90792)" length="5"/></item>"#;
        let r = super::parse_feed(xml).unwrap();
        assert_eq!(r.build, 90935);
        assert_eq!(r.version, "1.57.1");
        assert_eq!(r.length, 732995488);
    }
}
