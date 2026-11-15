//! Didia Keeper: builds Didia (the official Dia browser with its AI and Atlassian features
//! switched off) from official Dia releases, installs it as /Applications/Didia.app and
//! keeps it up to date.
//!
//!   didia-keeper                 open the window
//!   didia-keeper --rebuild       rebuild Didia from the official copy and install it
//!   didia-keeper --update        check Dia's feed now and build a newer release if there is one
//!   didia-keeper --background    what the background updater runs (every 10 minutes;
//!                                it checks Dia's feed at most every 4 hours)
//!   didia-keeper --enable-updates  turn on automatic updates
//!   didia-keeper --make-default  make Didia the default web browser
//!   didia-keeper --launch        build Didia if needed, then open it
//!   didia-keeper --help          this list
//!   didia-keeper --screenshot F  draw about 120 frames, save the window to F (PNG), quit
mod flags;
mod keeper;
mod macho;

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke};
use keeper::{MadeBy, Progress, Settings, Shared};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const HELP: &str = "Didia Keeper: makes Didia (Dia without AI and Atlassian) and keeps it up to date.

  didia-keeper                   open the window
  didia-keeper --rebuild         rebuild Didia and install it
  didia-keeper --update          check for a newer Dia now and build it
  didia-keeper --background      what automatic updates run
  didia-keeper --enable-updates  turn on automatic updates
  didia-keeper --make-default    make Didia the default browser
  didia-keeper --launch          build Didia if needed, then open it
  didia-keeper --help            show this";

fn fail(e: impl std::fmt::Display) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    if has("--help") || has("-h") {
        println!("{HELP}");
        return;
    }
    if has("--make-default") {
        match keeper::make_default_browser() {
            Ok(m) => println!("{m}"),
            Err(e) => fail(e),
        }
        return;
    }
    if has("--enable-updates") {
        match keeper::enable_background_updates() {
            Ok(()) => println!("automatic updates on"),
            Err(e) => fail(e),
        }
    }
    if has("--launch") {
        let s = Settings::load();
        keeper::refresh_agent_copy();
        if !keeper::installed_matches(&s) {
            let code = cli(&s, true, false);
            if code != 0 {
                std::process::exit(code);
            }
        }
        if keeper::staged_app().exists() {
            if let Err(e) = keeper::quit_didia().and_then(|_| keeper::install_staged()) {
                fail(e);
            }
        }
        match keeper::open_didia() {
            Ok(m) => println!("{m}"),
            Err(e) => fail(e),
        }
        return;
    }
    if has("--background") || has("--update") || has("--rebuild") {
        if !has("--background") {
            keeper::refresh_agent_copy();
        }
        std::process::exit(cli(&Settings::load(), has("--rebuild"), has("--update")));
    }
    // Testing aid: build Didia from <official Dia.app> into <folder> without installing it.
    if let Some(i) = args.iter().position(|a| a == "--build-to") {
        let (Some(src), Some(dir)) = (args.get(i + 1), args.get(i + 2)) else {
            eprintln!("usage: --build-to <official Dia.app> <folder>");
            std::process::exit(2);
        };
        let dest = Path::new(dir).join("Didia.app");
        let r = keeper::verify_official(Path::new(src))
            .and_then(|_| keeper::build_didia(Path::new(src), &dest, &Settings::load(), &None));
        match r {
            Ok(()) => println!("built {}", dest.display()),
            Err(e) => fail(e),
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

fn cli(s: &Settings, rebuild: bool, force: bool) -> i32 {
    let Some(_lock) = keeper::Lock::take() else {
        return 0; // the window (or another check) is already building
    };
    let r = if rebuild { keeper::rebuild_and_install(s, &None) } else { keeper::check_and_update(force, s, &None) };
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
        .with_inner_size([520.0, 600.0])
        .with_min_inner_size([460.0, 520.0]);
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
    original: Option<(PathBuf, keeper::AppInfo)>,
    pristine: Option<keeper::AppInfo>,
    staged: Option<keeper::AppInfo>,
    agent_on: bool,
    running: bool,
    default_browser: Option<PathBuf>,
    last_log: Option<String>,
}

impl Snapshot {
    fn read() -> Self {
        Snapshot {
            installed: keeper::app_info(&keeper::install_path()),
            original: keeper::official_dia_on_mac(),
            pristine: keeper::app_info(&keeper::pristine_app()),
            staged: keeper::app_info(&keeper::staged_app()),
            agent_on: keeper::agent_installed(),
            running: keeper::is_running(),
            default_browser: keeper::default_browser(),
            last_log: keeper::log_tail(1).pop(),
        }
    }
}

struct KeeperApp {
    settings: Settings,
    snap: Snapshot,
    snap_age: f64,
    progress: Shared,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_requested: bool,
}

#[derive(Clone, Copy)]
enum Job {
    Peek,
    Rebuild,
    Install,
    MakeDefault,
}

impl KeeperApp {
    fn new(screenshot: Option<PathBuf>) -> Self {
        if screenshot.is_none() {
            keeper::refresh_agent_copy();
        }
        let app = KeeperApp {
            settings: Settings::load(),
            snap: Snapshot::read(),
            snap_age: 0.0,
            progress: Arc::new(Mutex::new(Progress::default())),
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
            g.last_result = None;
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
                    None => Err("An automatic update is running; try again in a few minutes.".into()),
                    Some(_lock) => match job {
                        Job::Rebuild => keeper::rebuild_and_install(&s, &Some(p.clone())),
                        Job::Install => keeper::install_staged().map(|v| format!("Didia {v} is installed.")),
                        Job::MakeDefault => keeper::make_default_browser(),
                        Job::Peek => unreachable!(),
                    },
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

impl eframe::App for KeeperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_screenshot(&ctx);
        let now = ctx.input(|i| i.time);
        if now - self.snap_age > 2.0 {
            self.snap = Snapshot::read();
            self.snap_age = now;
        }
        let prog = self.progress.lock().unwrap().clone();
        ctx.request_repaint_after(std::time::Duration::from_millis(if prog.busy { 250 } else { 1500 }));

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(22))).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                self.header(ui);
                self.status_card(ui, &prog);
                self.options_card(ui);
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
                ui.label(RichText::new("Dia without the AI. Made from the official Dia and kept up to date.").color(MUTED));
            });
        });
        ui.add_space(14.0);
    }

    fn status_card(&mut self, ui: &mut egui::Ui, prog: &Progress) {
        let snap = &self.snap;
        let installed = snap.installed.as_ref();
        let latest = prog.latest.as_ref();
        let outdated = installed.zip(latest).map(|(i, l)| l.build > i.build).unwrap_or(false);
        let needs_rebuild = installed.map(|i| !matches!(i.made_by, MadeBy::Keeper { .. })).unwrap_or(false)
            || (installed.is_some() && !keeper::installed_matches(&self.settings));
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Didia").size(17.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, color) = match installed {
                        None => ("Not built yet", BAD),
                        Some(_) if needs_rebuild || outdated => ("Update available", WARN),
                        Some(_) => ("Up to date", GOOD),
                    };
                    pill(ui, text, color);
                    if snap.running {
                        pill(ui, "Open now", ACCENT);
                    }
                });
            });
            ui.add_space(4.0);
            egui::Grid::new("status").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new("Dia version found").color(MUTED));
                ui.label(match (&snap.original, &snap.pristine, latest) {
                    (Some((_, i)), _, _) => format!("{} (on this Mac)", i.version),
                    (None, Some(i), _) => format!("{} (kept copy)", i.version),
                    (None, None, Some(r)) => format!("{} (newest official)", r.version),
                    _ if prog.busy => "checking…".into(),
                    _ => "none on this Mac; it will be downloaded".into(),
                });
                ui.end_row();
                ui.label(RichText::new("Didia installed").color(MUTED));
                ui.label(match installed {
                    Some(i) => i.version.clone(),
                    None => "—".into(),
                });
                ui.end_row();
                if let Some(s) = &snap.staged {
                    ui.label(RichText::new("Waiting to install").color(MUTED));
                    ui.label(RichText::new(format!("{} — installs when you quit Didia", s.version)).color(WARN));
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
                    let label = if installed.is_some() { "Rebuild now" } else { "Build Didia" };
                    if ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(ACCENT)).clicked() {
                        self.start(Job::Rebuild);
                    }
                    if snap.staged.is_some() && !snap.running && ui.button("Install now").clicked() {
                        self.start(Job::Install);
                    }
                });
            }
            // One short line: the problem, the last result, or the last log entry.
            ui.add_space(2.0);
            if let Some(e) = &prog.error {
                ui.label(RichText::new(e).color(BAD));
            } else if let Some(m) = &prog.last_result {
                ui.label(RichText::new(m).color(GOOD));
            } else if let Some(l) = &snap.last_log {
                ui.label(RichText::new(l).monospace().size(11.5).color(MUTED));
            }
        });
    }

    fn options_card(&mut self, ui: &mut egui::Ui) {
        let mut toggle = None;
        let mut make_default = false;
        let mut changed = false;
        let snap = &self.snap;
        let is_default = snap.default_browser.as_deref() == Some(keeper::install_path().as_path());
        let installed = snap.installed.is_some();
        card(ui, |ui| {
            let mut on = snap.agent_on;
            if ui.checkbox(&mut on, RichText::new("Automatic updates").strong()).changed() {
                toggle = Some(on);
            }
            ui.label(RichText::new("When Dia has a new version, Didia is rebuilt from it and installed the next time you quit Didia.").color(MUTED));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if is_default {
                    pill(ui, "Didia is your default browser", GOOD);
                } else if ui.add_enabled(installed, egui::Button::new("Make default browser")).on_hover_text("macOS asks you to confirm").clicked() {
                    make_default = true;
                }
            });
            ui.add_space(8.0);
            changed |= ui
                .checkbox(&mut self.settings.applescript_js, "Allow JavaScript from AppleScript (for automation tools)")
                .changed();
            if changed {
                ui.label(RichText::new("Press “Rebuild now” to apply this.").color(WARN));
            }
        });
        if changed {
            self.settings.save();
        }
        if make_default {
            self.start(Job::MakeDefault);
        }
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
}
