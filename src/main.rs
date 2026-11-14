//! Didia Keeper: builds, from official Dia releases, and keeps up to date either
//!   Didia: the Dia browser with its AI and Atlassian features switched off, or
//!   Clia:  Dia's AI kept, Atlassian removed, Dia's agent run on your own Claude subscription.
//!
//!   didia-keeper                 open the window
//!   didia-keeper --background    what the background updater runs (every 10 minutes;
//!                                it checks Dia's feed at most every 4 hours)
//!   didia-keeper --update        check now and build a newer release if there is one
//!   didia-keeper --rebuild       rebuild from the official copy with the current settings
//!   didia-keeper --edition clia  (with the above) use this edition instead of the saved one
//!   didia-keeper --select clia   save the edition to build (didia or clia)
//!   didia-keeper --connect-claude  sign in with `claude setup-token` in Terminal and save the token
//!   didia-keeper --enable-updates  turn on automatic updates
//!   didia-keeper --screenshot F  draw about 120 frames, save the window to F (PNG), quit
mod byo;
mod flags;
mod keeper;
mod macho;

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke};
use keeper::{Edition, MadeBy, Progress, Settings, Shared};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Saved settings, with `--edition didia|clia` from the command line applied.
fn settings_from_args(args: &[String]) -> Settings {
    let mut s = Settings::load();
    if let Some(e) = args.iter().position(|a| a == "--edition").and_then(|i| args.get(i + 1)) {
        s.edition = if e.eq_ignore_ascii_case("clia") { Edition::Clia } else { Edition::Didia };
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    if let Some(e) = args.iter().position(|a| a == "--select").and_then(|i| args.get(i + 1)) {
        let mut s = Settings::load();
        s.edition = if e.eq_ignore_ascii_case("clia") { Edition::Clia } else { Edition::Didia };
        s.save();
        keeper::log(&format!("switched to {}", s.edition.name()));
        println!("edition: {}", s.edition.name());
    }
    if has("--connect-claude") {
        std::process::exit(connect_claude());
    }
    if has("--enable-updates") {
        match keeper::enable_background_updates() {
            Ok(()) => println!("automatic updates on"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    if has("--background") || has("--update") || has("--rebuild") {
        if !has("--background") {
            keeper::refresh_agent_copy();
        }
        std::process::exit(cli(settings_from_args(&args), has("--rebuild"), has("--update")));
    }
    // Testing aid: build the edition from <official Dia.app> into <folder> without installing it.
    if let Some(i) = args.iter().position(|a| a == "--build-to") {
        let (Some(src), Some(dir)) = (args.get(i + 1), args.get(i + 2)) else {
            eprintln!("usage: --build-to <official Dia.app> <folder>");
            std::process::exit(2);
        };
        let s = settings_from_args(&args);
        let dest = Path::new(dir).join(format!("{}.app", s.edition.name()));
        let r = keeper::verify_official(Path::new(src))
            .and_then(|_| keeper::build_edition(Path::new(src), &dest, &s, &None));
        match r {
            Ok(()) => println!("built {}", dest.display()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    run_window(screenshot);
}

/// Open Terminal with `claude setup-token` and wait (up to 10 minutes) for the token.
fn connect_claude() -> i32 {
    if let Err(e) = byo::start_setup_token() {
        eprintln!("{e}");
        return 1;
    }
    println!("Terminal opened: sign in with your Claude subscription there.");
    for _ in 0..600 {
        if byo::collect_setup_token().is_some() {
            println!("Claude connected.");
            return 0;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    eprintln!("No token yet. Paste it into Didia Keeper when you have it.");
    1
}

fn cli(s: Settings, rebuild: bool, force: bool) -> i32 {
    let Some(_lock) = keeper::Lock::take() else {
        return 0; // the window (or another check) is already building
    };
    let r = if rebuild { keeper::rebuild_and_install(&s, &None) } else { keeper::check_and_update(force, &s, &None) };
    match r {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(e) => {
            keeper::log(&format!("error: {e}"));
            eprintln!("{e}");
            1
        }
    }
}

fn run_window(screenshot: Option<PathBuf>) {
    let quiet = screenshot.is_some();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Didia Keeper")
        .with_inner_size([780.0, 700.0])
        .with_min_inner_size([620.0, 520.0]);
    if quiet {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if quiet {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    let _ = eframe::run_native(
        "Didia Keeper",
        options,
        Box::new(move |cc| {
            style(&cc.egui_ctx);
            Ok(Box::new(KeeperApp::new(screenshot)))
        }),
    );
}

// Soft, light look in the spirit of Dia's own settings windows.
const BG: Color32 = Color32::from_rgb(246, 245, 250);
const CARD: Color32 = Color32::WHITE;
const INK: Color32 = Color32::from_rgb(28, 27, 34);
const MUTED: Color32 = Color32::from_rgb(110, 108, 122);
const ACCENT: Color32 = Color32::from_rgb(98, 84, 230);
const GOOD: Color32 = Color32::from_rgb(34, 150, 96);
const WARN: Color32 = Color32::from_rgb(214, 128, 24);
const BAD: Color32 = Color32::from_rgb(206, 60, 60);

fn style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::light());
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(14.0, 7.0);
        s.visuals.panel_fill = BG;
        s.visuals.override_text_color = Some(INK);
        s.visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
        s.visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
        s.visuals.widgets.active.corner_radius = CornerRadius::same(8);
        s.visuals.selection.bg_fill = ACCENT;
        for (t, size) in [
            (egui::TextStyle::Body, 14.0),
            (egui::TextStyle::Button, 14.0),
            (egui::TextStyle::Heading, 24.0),
            (egui::TextStyle::Small, 12.0),
        ] {
            if let Some(f) = s.text_styles.get_mut(&t) {
                f.size = size;
            }
        }
    });
}

struct Snapshot {
    installed: Option<keeper::AppInfo>,
    pristine: Option<keeper::AppInfo>,
    original: Option<(PathBuf, keeper::AppInfo)>,
    staged: Option<keeper::AppInfo>,
    agent_on: bool,
    running: bool,
    other_running: bool,
    has_token: bool,
    last_check: Option<std::time::SystemTime>,
    log: Vec<String>,
}

impl Snapshot {
    fn read(e: Edition) -> Self {
        if !byo::has_token() && byo::setup_transcript().exists() {
            byo::collect_setup_token();
        }
        Snapshot {
            installed: keeper::app_info(&e.install_path()),
            pristine: keeper::app_info(&keeper::pristine_app()),
            original: keeper::official_dia_on_mac(),
            staged: keeper::app_info(&keeper::staged_app(e)),
            agent_on: keeper::agent_installed(),
            running: keeper::is_running(e),
            other_running: keeper::is_running(e.other()),
            has_token: byo::has_token(),
            last_check: keeper::last_check(),
            log: keeper::log_tail(40),
        }
    }
}

struct KeeperApp {
    settings: Settings,
    token_input: String,
    snap: Snapshot,
    snap_age: f64,
    progress: Shared,
    show_ai_list: bool,
    show_atl_list: bool,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_requested: bool,
}

impl KeeperApp {
    fn new(screenshot: Option<PathBuf>) -> Self {
        let settings = Settings::load();
        if screenshot.is_none() {
            keeper::refresh_agent_copy();
        }
        let snap = Snapshot::read(settings.edition);
        let app = KeeperApp {
            settings,
            token_input: String::new(),
            snap,
            snap_age: 0.0,
            progress: Arc::new(Mutex::new(Progress::default())),
            show_ai_list: false,
            show_atl_list: false,
            screenshot,
            frames: 0,
            shot_requested: false,
        };
        if app.screenshot.is_none() {
            app.start(Job::Peek);
        }
        app
    }

    fn start(&self, job: Job) {
        {
            let mut g = self.progress.lock().unwrap();
            if g.busy {
                return;
            }
            g.busy = true;
            g.error = None;
            g.step = "Starting".into();
            g.fraction = None;
        }
        let p = self.progress.clone();
        let s = self.settings.clone();
        std::thread::spawn(move || {
            let result = match job {
                Job::Peek => keeper::fetch_latest().map(|r| {
                    p.lock().unwrap().latest = Some(r);
                    String::new()
                }),
                _ => match keeper::Lock::take() {
                    None => Err("A background update is running; try again in a few minutes.".into()),
                    Some(_lock) => {
                        let sp = Some(p.clone());
                        match job {
                            Job::Update => keeper::check_and_update(true, &s, &sp),
                            Job::Rebuild => keeper::rebuild_and_install(&s, &sp),
                            Job::Install => keeper::install_staged(s.edition).map(|v| format!("{} {v} is installed.", s.edition.name())),
                            Job::Peek => unreachable!(),
                        }
                    }
                },
            };
            let mut g = p.lock().unwrap();
            g.busy = false;
            g.fraction = None;
            match result {
                Ok(m) if !m.is_empty() => g.last_result = Some(m),
                Ok(_) => {}
                Err(e) => {
                    keeper::log(&format!("error: {e}"));
                    g.error = Some(e)
                }
            }
        });
    }
}

#[derive(Clone, Copy)]
enum Job {
    Peek,
    Update,
    Rebuild,
    Install,
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, Color32::from_rgb(230, 228, 238)))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
    ui.add_space(10.0);
}

fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| ui.label(RichText::new(text).color(color).size(12.5).strong()));
}

fn ago(t: std::time::SystemTime) -> String {
    let s = std::time::SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86399 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86400),
    }
}

impl eframe::App for KeeperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_screenshot(&ctx);
        let now = ctx.input(|i| i.time);
        if now - self.snap_age > 2.0 {
            self.snap = Snapshot::read(self.settings.edition);
            self.snap_age = now;
        }
        let prog = self.progress.lock().unwrap().clone();
        ctx.request_repaint_after(std::time::Duration::from_millis(if prog.busy { 250 } else { 1500 }));

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(22))).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                self.header(ui);
                self.status_card(ui, &prog);
                self.edition_card(ui);
                self.updates_card(ui);
                match self.settings.edition {
                    Edition::Didia => self.removed_card(ui),
                    Edition::Clia => self.clia_card(ui),
                }
                self.activity_card(ui);
            });
        });
    }
}

impl KeeperApp {
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let [w, h] = img.size;
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, raw) {
                let _ = buf.save(&path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, CornerRadius::same(12), ACCENT);
            p.circle_filled(rect.center(), 13.0, Color32::from_rgb(196, 188, 255));
            p.circle_filled(rect.center(), 6.0, Color32::WHITE);
            ui.vertical(|ui| {
                ui.label(RichText::new("Didia Keeper").heading().strong());
                ui.label(
                    RichText::new("Dia without the bloat, rebuilt from every official release: no AI at all, or your own Claude instead of Dia's.")
                        .color(MUTED),
                );
            });
        });
        ui.add_space(14.0);
    }

    fn status_card(&mut self, ui: &mut egui::Ui, prog: &Progress) {
        let snap = &self.snap;
        let name = self.settings.edition.name();
        let pending = self.pending_rebuild();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Your {name}")).size(17.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let latest = prog.latest.as_ref().map(|r| r.build);
                    let inst = snap.installed.as_ref();
                    let (text, color) = match inst {
                        None => ("Not installed", BAD),
                        Some(i) if !matches!(i.made_by, MadeBy::Keeper { .. }) || pending => ("Needs a rebuild", WARN),
                        Some(i) if latest.map(|l| l > i.build).unwrap_or(false) => ("Update available", WARN),
                        Some(_) if latest.is_some() => ("Up to date", GOOD),
                        Some(_) => ("Installed", GOOD),
                    };
                    pill(ui, text, color);
                    if snap.running {
                        pill(ui, "Open now", ACCENT);
                    }
                });
            });
            ui.add_space(4.0);
            egui::Grid::new("status").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new("Installed").color(MUTED));
                ui.label(match &snap.installed {
                    Some(i) => format!("{name} {}  (build {})", i.version, i.build),
                    None => "—".into(),
                });
                ui.end_row();
                ui.label(RichText::new("Made by").color(MUTED));
                ui.label(match snap.installed.as_ref().map(|i| &i.made_by) {
                    Some(MadeBy::Keeper { own_claude: true, .. }) => "Didia Keeper — AI on your own Claude, Atlassian off".into(),
                    Some(MadeBy::Keeper { ai_off, atlassian_off, .. }) => format!(
                        "Didia Keeper — AI {}, Atlassian {}",
                        if *ai_off { "off" } else { "on" },
                        if *atlassian_off { "off" } else { "on" }
                    ),
                    Some(MadeBy::Official) => "plain official Dia".into(),
                    _ => "another tool (rebuild to switch AI and Atlassian off)".into(),
                });
                ui.end_row();
                ui.label(RichText::new("Newest official Dia").color(MUTED));
                ui.label(match &prog.latest {
                    Some(r) => format!("{}  (build {})", r.version, r.build),
                    None if prog.busy => "checking…".into(),
                    None => "not checked yet".into(),
                });
                ui.end_row();
                ui.label(RichText::new("Official copy kept").color(MUTED));
                ui.label(match &snap.pristine {
                    Some(i) => format!("Dia {} — signature checked", i.version),
                    None => "none yet (made on first build)".into(),
                });
                ui.end_row();
                ui.label(RichText::new("Original Dia app").color(MUTED));
                ui.label(match &snap.original {
                    Some((path, i)) => format!("Dia {} at {} — used as the source, left untouched", i.version, path.display()),
                    None => "not on this Mac — official releases are downloaded instead".into(),
                });
                ui.end_row();
                if let Some(s) = &snap.staged {
                    ui.label(RichText::new("Waiting to install").color(MUTED));
                    ui.label(RichText::new(format!("{name} {} — installs when {name} is closed", s.version)).color(WARN));
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            if prog.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&prog.step);
                });
                if let Some(f) = prog.fraction {
                    ui.add(egui::ProgressBar::new(f).fill(ACCENT).show_percentage());
                }
            } else {
                ui.horizontal_wrapped(|ui| {
                    if ui.add(egui::Button::new(RichText::new("Check for updates").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                        self.start(Job::Update);
                    }
                    if ui.button(format!("Rebuild {name} now")).on_hover_text("Rebuild from the official copy with the settings below").clicked() {
                        self.start(Job::Rebuild);
                    }
                    if snap.staged.is_some() && !snap.running && ui.button("Install now").clicked() {
                        self.start(Job::Install);
                    }
                    if snap.installed.is_some() && ui.button(format!("Open {name}")).clicked() {
                        let _ = std::process::Command::new("/usr/bin/open").arg(self.settings.edition.install_path()).spawn();
                    }
                });
            }
            if snap.other_running {
                let other = self.settings.edition.other().name();
                ui.label(RichText::new(format!("{other} is open. Both share Dia's profile, so quit {other} before opening {name}.")).color(WARN));
            }
            if let Some(e) = &prog.error {
                ui.label(RichText::new(e).color(BAD));
            } else if let Some(m) = &prog.last_result {
                ui.label(RichText::new(m).color(GOOD));
            }
        });
    }

    fn removed_card(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        card(ui, |ui| {
            ui.label(RichText::new("What's taken out").size(17.0).strong());
            ui.label(RichText::new("Didia switches these off with Dia's own feature switches, and hides any button that still shows.").color(MUTED));
            ui.add_space(6.0);
            changed |= ui.checkbox(&mut self.settings.remove_ai, RichText::new("AI features").strong()).changed();
            ui.indent("ai", |ui| {
                ui.label(RichText::new("Chat and “Chat with this tab”, Ask on page, Skills, agents and browse-for-me, Morning Brief, memory and personalization, AI tab clean-up and renaming, AI upsells and trials.").color(MUTED));
                let label = if self.show_ai_list { "Hide the switches" } else { format!("Show the {} switches", flags::AI_FLAGS.len()).leak() as &str };
                if ui.link(label).clicked() {
                    self.show_ai_list = !self.show_ai_list;
                }
                if self.show_ai_list {
                    ui.label(RichText::new(flags::AI_FLAGS.join(", ")).monospace().size(11.5).color(MUTED));
                }
            });
            ui.add_space(4.0);
            changed |= ui.checkbox(&mut self.settings.remove_atlassian, RichText::new("Atlassian").strong()).changed();
            ui.indent("atl", |ui| {
                ui.label(RichText::new("Atlassian sign-in and sign-up, Jira and Confluence connections, Atlassian tools and onboarding step, Atlassian account buttons.").color(MUTED));
                let label = if self.show_atl_list { "Hide the switches" } else { format!("Show the {} switches", flags::ATLASSIAN_FLAGS.len() + 1).leak() as &str };
                if ui.link(label).clicked() {
                    self.show_atl_list = !self.show_atl_list;
                }
                if self.show_atl_list {
                    ui.label(RichText::new(flags::ATLASSIAN_FLAGS.join(", ") + ", atlassian-identity-signup-rollout-percentage").monospace().size(11.5).color(MUTED));
                }
            });
            ui.add_space(4.0);
            changed |= ui
                .checkbox(&mut self.settings.applescript_js, "Allow JavaScript from AppleScript (only needed for automation tools)")
                .changed();
            if self.pending_rebuild() {
                ui.add_space(4.0);
                ui.label(RichText::new("Press “Rebuild Didia now” to apply these choices.").color(WARN));
            }
        });
        if changed {
            self.settings.save();
        }
    }

    fn updates_card(&mut self, ui: &mut egui::Ui) {
        let snap = &self.snap;
        let mut toggle = None;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Automatic updates").size(17.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut on = snap.agent_on;
                    let text = if on { "On" } else { "Off" };
                    if ui.add(egui::Checkbox::new(&mut on, text)).changed() {
                        toggle = Some(on);
                    }
                });
            });
            let name = self.settings.edition.name();
            ui.label(RichText::new(format!("Checks Dia's official release feed every 4 hours, downloads new releases, refuses anything not signed by The Browser Company, rebuilds {name} and installs it the next time {name} is closed.")).color(MUTED));
            if let Some(t) = snap.last_check {
                ui.label(RichText::new(format!("Last checked {}", ago(t))).color(MUTED).size(12.5));
            }
        });
        match toggle {
            Some(true) => {
                if let Err(e) = keeper::enable_background_updates() {
                    self.progress.lock().unwrap().error = Some(e);
                }
                self.snap_age = -10.0;
            }
            Some(false) => {
                let _ = keeper::disable_background_updates();
                self.snap_age = -10.0;
            }
            None => {}
        }
    }

    /// The installed app was built with different choices than the ones now set.
    fn pending_rebuild(&self) -> bool {
        let s = &self.settings;
        let (ai, atl, _) = s.switches();
        match self.snap.installed.as_ref().map(|i| &i.made_by) {
            Some(MadeBy::Keeper { ai_off, atlassian_off, own_claude, .. }) => {
                *ai_off != ai || *atlassian_off != atl || *own_claude != (s.edition == Edition::Clia && s.own_claude)
            }
            Some(_) => true,
            None => false,
        }
    }

    fn edition_card(&mut self, ui: &mut egui::Ui) {
        let mut pick = None;
        card(ui, |ui| {
            ui.label(RichText::new("Which browser").size(17.0).strong());
            ui.add_space(2.0);
            ui.columns(2, |cols| {
                for (col, e, title, text) in [
                    (0, Edition::Didia, "Didia", "No AI at all. Chat, agents, Morning Brief and Atlassian are switched off."),
                    (1, Edition::Clia, "Clia", "Dia's AI on your own Claude subscription. Atlassian removed, no Dia plan prompts."),
                ] {
                    let ui = &mut cols[col];
                    let on = self.settings.edition == e;
                    let r = egui::Frame::new()
                        .fill(if on { ACCENT.gamma_multiply(0.10) } else { BG })
                        .stroke(Stroke::new(if on { 2.0 } else { 1.0 }, if on { ACCENT } else { Color32::from_rgb(225, 223, 233) }))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(14))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(title).size(16.0).strong().color(if on { ACCENT } else { INK }));
                                if on {
                                    ui.label(RichText::new("selected").size(12.0).color(ACCENT));
                                }
                            });
                            ui.label(RichText::new(text).color(MUTED));
                            });
                        })
                        .response
                        .interact(egui::Sense::click());
                    if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !on {
                        pick = Some(e);
                    }
                }
            });
            ui.label(
                RichText::new("Both are made from the same official Dia and share its profile, so your tabs and logins carry over. Use one at a time.")
                    .color(MUTED)
                    .size(12.5),
            );
        });
        if let Some(e) = pick {
            self.settings.edition = e;
            self.settings.save();
            keeper::log(&format!("switched to {}", e.name()));
            self.snap = Snapshot::read(e);
        }
    }

    fn clia_card(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let mut msg: Option<Result<String, String>> = None;
        card(ui, |ui| {
            ui.label(RichText::new("Your own Claude").size(17.0).strong());
            ui.label(RichText::new("Dia's AI already runs on a built-in copy of Claude Code. Clia hands it your own Claude subscription (Pro or Max) and lets it reach Anthropic directly instead of Dia's servers. Dia's tools — tabs, pages, apps — keep working.").color(MUTED));
            ui.add_space(4.0);
            changed |= ui.checkbox(&mut self.settings.own_claude, RichText::new("Use my own Claude subscription").strong()).changed();
            if self.settings.own_claude {
                ui.indent("tok", |ui| {
                    if self.snap.has_token {
                        ui.horizontal(|ui| {
                            pill(ui, "Claude connected", GOOD);
                            if ui.small_button("Disconnect").clicked() {
                                byo::clear_token();
                                self.snap_age = -10.0;
                            }
                        });
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            pill(ui, "Not connected", WARN);
                            if ui.add(egui::Button::new(RichText::new("Connect my Claude subscription").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                                msg = Some(byo::start_setup_token().map(|_| "Terminal opened: sign in there; Clia picks up the token by itself.".into()));
                            }
                        });
                        ui.label(RichText::new("This runs `claude setup-token` (Claude Code must be installed). Or paste a token from it here:").color(MUTED).size(12.5));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.token_input).password(true).hint_text("sk-ant-oat01-…").desired_width(320.0));
                            if ui.button("Save").clicked() {
                                msg = Some(byo::save_token(&self.token_input).map(|_| "Claude connected.".into()));
                                self.token_input.clear();
                                self.snap_age = -10.0;
                            }
                        });
                    }
                    ui.label(RichText::new("Codex isn't possible: Dia's agent only runs Claude Code.").color(MUTED).size(12.5));
                });
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Atlassian is always removed in Clia (sign-in, Jira and Confluence, Atlassian tools).").color(MUTED));
            changed |= ui
                .checkbox(&mut self.settings.applescript_js, "Allow JavaScript from AppleScript (only needed for automation tools)")
                .changed();
            if self.pending_rebuild() {
                ui.label(RichText::new("Press “Rebuild Clia now” to apply these choices.").color(WARN));
            }
        });
        if changed {
            self.settings.save();
        }
        if let Some(m) = msg {
            let mut g = self.progress.lock().unwrap();
            match m {
                Ok(t) => {
                    g.error = None;
                    g.last_result = Some(t)
                }
                Err(e) => g.error = Some(e),
            }
        }
    }

    fn activity_card(&self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Activity").size(17.0).strong());
            if self.snap.log.is_empty() {
                ui.label(RichText::new("Nothing yet.").color(MUTED));
            }
            egui::ScrollArea::vertical().max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
                for l in &self.snap.log {
                    ui.label(RichText::new(l).monospace().size(11.5).color(MUTED));
                }
            });
        });
    }
}
