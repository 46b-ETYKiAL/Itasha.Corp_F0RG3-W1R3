//! Itasha.Corp provisioning console — a NERV/MAGI x the-wired install handshake.
//! Reusable across every Itasha.Corp Windows app: all app-specific data (name,
//! binary, vendor, install subdir, voice colour, kanji, tagline) + the payload
//! are injected at build time (see build.rs). Influences are expressed as
//! motifs (Lain / GitS / Akira / Eva / Gundam / JDM / Gharliera / antireal).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod theme;

mod config {
    include!(concat!(env!("OUT_DIR"), "/app_config.rs"));
}

/// The app payload (compiled binary + assets), zipped, embedded at build time.
static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.zip"));

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

fn default_dir() -> String {
    format!(
        r"C:\Program Files\{}\{}",
        config::VENDOR,
        config::INSTALL_SUBDIR
    )
}

#[derive(PartialEq, Clone, Copy)]
enum Phase {
    Boot,
    Configure,
    Provision,
    Online,
    Failed,
}

struct App {
    phase: Phase,
    boot_done_at: Option<f64>,
    dir: String,
    start_menu: bool,
    desktop: bool,
    add_path: bool,
    launch: bool,
    frac: f32,
    log: Vec<(String, bool)>, // (line, done)
    step_rx: Option<Receiver<engine::Step>>,
    done_rx: Option<Receiver<Result<(), String>>>,
    error: Option<String>,
    /// `ctx.input(..).time` at the instant we entered `Phase::Online`. Used to
    /// hold the "NODE ONLINE" frame briefly before the auto-launch fires.
    online_at: Option<f64>,
    /// Set once the installed app has been spawned, so auto-launch fires exactly
    /// once (the `update`/`pump` loop runs every frame).
    launched: bool,
    /// `true` once the window has been explicitly placed (centred) AND revealed.
    /// The window starts HIDDEN and is positioned + shown on the first frame the
    /// real monitor geometry is known — see `center_and_reveal`. Builder-time
    /// placement (`centered: true` / `with_position`) is unreliable on Windows
    /// multi-monitor / mixed-DPI because the window↔monitor DPI relationship is
    /// not resolved until after creation (emilk/egui #4918); placing it once the
    /// viewport reports its monitor size is the correct, DPI-accurate lifecycle.
    window_placed: bool,
    /// Frames elapsed waiting for the monitor size to be reported. Bounds the
    /// hidden window: if the size never arrives we reveal anyway (a possibly
    /// OS-placed window is always better than one stuck invisible).
    place_attempts: u32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            phase: Phase::Boot,
            boot_done_at: None,
            dir: default_dir(),
            // Start-Menu entry ON by default (the expected place to find an app);
            // Desktop shortcut OFF by default (opt-in to avoid desktop clutter).
            start_menu: true,
            desktop: false,
            add_path: false,
            launch: true,
            frac: 0.0,
            log: Vec::new(),
            step_rx: None,
            done_rx: None,
            error: None,
            online_at: None,
            launched: false,
            window_placed: false,
            place_attempts: 0,
        }
    }
}

/// Centred outer position (top-left, in egui points) for a `win`-sized window
/// on a `monitor`-sized display. Clamped to ≥ 0 on each axis so a window larger
/// than the monitor lands at the origin rather than partly off-screen. Pure, so
/// the placement math is unit-tested without a live window.
fn centered_outer_position(monitor: egui::Vec2, win: egui::Vec2) -> egui::Pos2 {
    egui::pos2(
        ((monitor.x - win.x) * 0.5).max(0.0),
        ((monitor.y - win.y) * 0.5).max(0.0),
    )
}

impl App {
    /// Place the window centred on its monitor, then reveal it — done ONCE, on
    /// the first frame the viewport reports a real monitor size. The window is
    /// created hidden (`with_visible(false)`) so the user never sees an
    /// off-centre flash: we compute the centred outer position from the monitor
    /// size and the window's own outer size (both in DPI-correct egui points),
    /// move the window there, and only then make it visible.
    ///
    /// This is the reliable replacement for eframe's builder-time `centered`
    /// flag, which mis-places the window on Windows multi-monitor / mixed-DPI
    /// setups because the window↔monitor DPI relationship is not resolved until
    /// after creation (emilk/egui #4918). It is bounded: if the monitor size is
    /// never reported within `MAX` frames we reveal the window anyway, so it can
    /// never be left invisible.
    fn center_and_reveal(&mut self, ctx: &egui::Context) {
        if self.window_placed {
            return;
        }
        const MAX_FRAMES: u32 = 30; // ~0.5s at 60 fps; update() repaints each frame
        self.place_attempts += 1;
        let monitor = ctx.input(|i| i.viewport().monitor_size);
        if let Some(monitor) = monitor.filter(|m| m.x > 1.0 && m.y > 1.0) {
            // Prefer the realised outer size (includes the title bar/borders);
            // fall back to the fixed inner size before the first outer_rect
            // report (the window is non-resizable, so this is exact enough).
            let win = ctx
                .input(|i| i.viewport().outer_rect)
                .map(|r| r.size())
                .unwrap_or_else(|| egui::vec2(900.0, 560.0));
            let pos = centered_outer_position(monitor, win);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            self.window_placed = true;
        } else if self.place_attempts >= MAX_FRAMES {
            // Monitor size never arrived — reveal anyway; never stay hidden.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            self.window_placed = true;
        }
    }

    fn start_install(&mut self) {
        let (stx, srx) = mpsc::channel();
        let (dtx, drx) = mpsc::channel();
        self.step_rx = Some(srx);
        self.done_rx = Some(drx);
        self.phase = Phase::Provision;
        let opts = engine::Opts {
            dir: PathBuf::from(self.dir.trim()),
            start_menu: self.start_menu,
            desktop: self.desktop,
            add_path: self.add_path,
        };
        std::thread::spawn(move || {
            let cb = |s: engine::Step| {
                let _ = stx.send(s);
            };
            let res = engine::install(&opts, PAYLOAD, &cb);
            let _ = dtx.send(res);
        });
    }

    /// Spawn the freshly-installed app (detached, DE-ELEVATED) and exit the
    /// installer. Launches the real installed binary under its install directory
    /// — never a temp/extract path.
    ///
    /// The installer self-elevates (`requireAdministrator`), so spawning the exe
    /// directly (`Command::new(exe).spawn()`) would hand the editor the
    /// installer's ADMIN token — a privilege a local file editor must never run
    /// with (an admin-token editor is a privilege-escalation foothold and writes
    /// files as SYSTEM/admin). Launching via `explorer.exe <exe>` makes Explorer,
    /// which runs at the normal MEDIUM integrity level, be the process that starts
    /// the app, so the app inherits the USER's token rather than the installer's.
    /// This is the zero-`unsafe` equivalent of NSIS `ShellExecAsUser` / Inno
    /// `runasoriginaluser`. (We drop the previous `current_dir(install_dir)` —
    /// Explorer controls the child's cwd — because the app resolves its config +
    /// assets from `current_exe()` / `%LOCALAPPDATA%`, not the working directory,
    /// so de-elevation costs nothing functionally.)
    ///
    /// `spawn()` returns immediately (no `wait`), so the installer never holds a
    /// handle that could block the app; we then exit 0 so the installer process
    /// is gone before the app's window paints.
    fn launch_and_exit(&self) -> ! {
        let install_dir = PathBuf::from(self.dir.trim());
        let exe = engine::installed_binary(&install_dir);
        let _ = std::process::Command::new("explorer.exe").arg(&exe).spawn();
        std::process::exit(0);
    }

    fn pump(&mut self) {
        if let Some(rx) = &self.step_rx {
            while let Ok(s) = rx.try_recv() {
                self.frac = s.frac;
                if let Some(last) = self.log.last_mut() {
                    last.1 = true;
                }
                self.log.push((s.label, false));
            }
        }
        if let Some(rx) = &self.done_rx {
            if let Ok(res) = rx.try_recv() {
                if let Some(last) = self.log.last_mut() {
                    last.1 = true;
                }
                match res {
                    Ok(()) => {
                        self.phase = Phase::Online;
                        // record the instant we came online; the update loop holds
                        // the completion frame briefly, then auto-launches if asked.
                        self.online_at = None;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.phase = Phase::Failed;
                    }
                }
                self.step_rx = None;
                self.done_rx = None;
            }
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        let c = theme::VOID;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Place + reveal the window centred on its monitor on the first frame the
        // monitor geometry is known (the window is created hidden). Reliable on
        // multi-monitor / mixed-DPI where builder-time centering is not.
        self.center_and_reveal(ctx);
        ctx.request_repaint(); // animate
        let t = ctx.input(|i| i.time);
        self.pump();

        // Auto-launch: the moment provisioning completes, if "Launch on finish"
        // is checked, spawn the installed app and exit WITHOUT requiring a click.
        // We hold the "NODE ONLINE" frame for a brief beat so the user sees the
        // completion state, then launch. `launched` guards single-fire.
        if self.phase == Phase::Online && self.launch && !self.launched {
            let started = *self.online_at.get_or_insert(t);
            if t - started >= 0.6 {
                self.launched = true;
                self.launch_and_exit();
            }
        }

        // boot auto-advance
        if self.phase == Phase::Boot {
            let start = *self.boot_done_at.get_or_insert(t + 1.8);
            if t >= start {
                self.phase = Phase::Configure;
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::VOID))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let p = ui.painter().clone();
                paint_grid(&p, rect, t);
                theme::paint_chrome(&p, rect, t);
                paint_strip(&p, rect);

                match self.phase {
                    Phase::Boot => self.ui_boot(ui, &p, rect, t),
                    Phase::Configure => self.ui_configure(ui, rect),
                    Phase::Provision | Phase::Online | Phase::Failed => {
                        self.ui_provision(ui, &p, rect, t)
                    }
                }
            });
    }
}

impl App {
    fn ui_boot(&self, _ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, t: f64) {
        let v = theme::voice();
        let c = rect.center();
        wordmark(p, Pos2::new(c.x, c.y - 64.0));
        p.text(
            Pos2::new(c.x, c.y - 18.0),
            Align2::CENTER_CENTER,
            config::APP_NAME,
            FontId::monospace(40.0),
            theme::TEXT,
        );
        let pulse = 0.5 + 0.5 * ((t * 3.0).sin() as f32);
        p.text(
            Pos2::new(c.x, c.y + 26.0),
            Align2::CENTER_CENTER,
            "ESTABLISHING LINK · SYNCHRONIZING",
            FontId::monospace(13.0),
            Color32::from_rgba_unmultiplied(v.r(), v.g(), v.b(), (110.0 + 130.0 * pulse) as u8),
        );
        // a sweeping sync bar
        let bar = Rect::from_center_size(Pos2::new(c.x, c.y + 58.0), Vec2::new(280.0, 4.0));
        p.rect_filled(bar, 2.0, theme::dimmed(v, 0.78));
        let sweep = ((t * 0.6).fract() as f32) * bar.width();
        p.rect_filled(
            Rect::from_min_size(bar.left_top(), Vec2::new(sweep, 4.0)),
            2.0,
            v,
        );
    }

    fn ui_configure(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let v = theme::voice();
        // left brand spine
        let split = rect.left() + rect.width() * 0.40;
        self.ui_spine(ui, rect, split);

        // right console: location + options + initiate
        let console = Rect::from_min_max(
            Pos2::new(split + 24.0, rect.top() + 40.0),
            Pos2::new(rect.right() - 40.0, rect.bottom() - 56.0),
        );
        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(console)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        ui.add_space(4.0);
        section_label(&mut ui, "PARTITION · designate install node");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.dir)
                    .desired_width(ui.available_width() - 96.0)
                    .font(egui::TextStyle::Monospace),
            );
            if ui.button("BROWSE").clicked() {
                if let Some(d) = pick_folder() {
                    self.dir =
                        format!(r"{}\{}", d.trim_end_matches('\\'), config::INSTALL_SUBDIR);
                }
            }
        });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("default · {}", default_dir()))
                .color(theme::DIM)
                .small(),
        );

        ui.add_space(22.0);
        section_label(&mut ui, "NODE CONFIG · integration");
        ui.add_space(6.0);
        opt_row(&mut ui, &mut self.start_menu, "Add to Start menu (recommended)", v);
        ui.label(
            egui::RichText::new(format!(
                "    creates a \"{}\" shortcut in the Start menu — search \"{}\" or \"Copland\" to launch",
                config::APP_NAME,
                config::APP_NAME
            ))
            .color(theme::DIM)
            .small(),
        );
        ui.add_space(4.0);
        opt_row(&mut ui, &mut self.desktop, "Desktop shortcut", v);
        opt_row(&mut ui, &mut self.add_path, "Add to system PATH", v);
        opt_row(&mut ui, &mut self.launch, format!("Launch {} on finish", config::APP_NAME), v);

        ui.add_space(20.0);
        // signing posture note (honest)
        ui.label(
            egui::RichText::new("⚠ unsigned preview build · SmartScreen may warn")
                .color(theme::AMBER)
                .small(),
        );

        // bottom-right INITIATE
        let btn_rect = Rect::from_min_max(
            Pos2::new(console.right() - 220.0, console.bottom() - 6.0),
            Pos2::new(console.right(), console.bottom() + 34.0),
        );
        let mut bui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(btn_rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let valid = !self.dir.trim().is_empty();
        if bui
            .add_enabled(valid, brand_button("INITIATE  ▸", v))
            .clicked()
        {
            self.start_install();
        }
    }

    fn ui_spine(&self, ui: &mut egui::Ui, rect: Rect, split: f32) {
        let p = ui.painter();
        let v = theme::voice();
        let x = rect.left() + 46.0;
        wordmark(p, Pos2::new(x + 86.0, rect.top() + 58.0));
        p.text(
            Pos2::new(x, rect.top() + 96.0),
            Align2::LEFT_CENTER,
            config::APP_NAME,
            FontId::monospace(34.0),
            theme::TEXT,
        );
        p.text(
            Pos2::new(x, rect.top() + 128.0),
            Align2::LEFT_CENTER,
            wrap(config::TAGLINE, 34),
            FontId::monospace(12.0),
            theme::MUTED,
        );
        // vertical telemetry spine (mecha/antireal HUD ticks)
        let sx = x + 2.0;
        let top = rect.top() + 168.0;
        let bot = rect.bottom() - 80.0;
        p.line_segment([Pos2::new(sx, top), Pos2::new(sx, bot)], Stroke::new(1.0, theme::HAIRLINE));
        let labels = [
            ("VENDOR", config::VENDOR),
            ("VERSION", config::VERSION),
            ("TARGET", "x86_64-pc-windows"),
            ("PUBLISHER", config::PUBLISHER),
        ];
        for (i, (k, val)) in labels.iter().enumerate() {
            let y = top + 14.0 + i as f32 * 30.0;
            p.circle_filled(Pos2::new(sx, y), 2.4, v);
            p.text(Pos2::new(sx + 14.0, y - 6.0), Align2::LEFT_CENTER, *k, FontId::monospace(9.5), theme::DIM);
            p.text(Pos2::new(sx + 14.0, y + 7.0), Align2::LEFT_CENTER, *val, FontId::monospace(11.5), theme::TEXT);
        }
        let _ = split;
    }

    fn ui_provision(&mut self, ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, t: f64) {
        let v = theme::voice();
        let done = self.phase == Phase::Online;
        let failed = self.phase == Phase::Failed;

        // left: sync gauge (Eva sync-ratio)
        let gauge_c = Pos2::new(rect.left() + rect.width() * 0.24, rect.center().y - 6.0);
        let shown = if done { 1.0 } else { self.frac };
        paint_gauge(p, gauge_c, 78.0, shown, v, t, failed);
        let big = if failed { "ERR".into() } else { format!("{:.0}%", shown * 100.0) };
        let col = if failed { theme::RED } else { theme::TEXT };
        p.text(gauge_c, Align2::CENTER_CENTER, big, FontId::monospace(30.0), col);
        let state = if done {
            "NODE ONLINE"
        } else if failed {
            "FAULT"
        } else {
            "PROVISIONING"
        };
        p.text(
            Pos2::new(gauge_c.x, gauge_c.y + 104.0),
            Align2::CENTER_CENTER,
            state,
            FontId::monospace(12.0),
            if failed { theme::RED } else { v },
        );

        // right: provisioning log
        let log_rect = Rect::from_min_max(
            Pos2::new(rect.left() + rect.width() * 0.46, rect.top() + 56.0),
            Pos2::new(rect.right() - 44.0, rect.bottom() - 70.0),
        );
        p.rect_filled(log_rect, 4.0, theme::PANEL);
        p.rect_stroke(log_rect, 4.0, Stroke::new(1.0, theme::HAIRLINE));
        let mut y = log_rect.top() + 16.0;
        p.text(Pos2::new(log_rect.left() + 14.0, y), Align2::LEFT_CENTER, "// provisioning log", FontId::monospace(10.0), theme::DIM);
        y += 20.0;
        for (line, ok) in self.log.iter().rev().take(10).collect::<Vec<_>>().iter().rev() {
            let mark = if *ok { "[OK]" } else { " .. " };
            let mc = if *ok { theme::GREEN } else { theme::MUTED };
            p.text(Pos2::new(log_rect.left() + 14.0, y), Align2::LEFT_CENTER, format!("> {line}"), FontId::monospace(12.0), theme::TEXT);
            p.text(Pos2::new(log_rect.right() - 18.0, y), Align2::RIGHT_CENTER, mark, FontId::monospace(12.0), mc);
            y += 20.0;
        }
        if let Some(err) = &self.error {
            p.text(Pos2::new(log_rect.left() + 14.0, log_rect.bottom() - 18.0), Align2::LEFT_CENTER, wrap(err, 52), FontId::monospace(11.0), theme::RED);
        }

        // finish controls
        if done || failed {
            let br = Rect::from_min_max(Pos2::new(rect.right() - 320.0, rect.bottom() - 48.0), Pos2::new(rect.right() - 40.0, rect.bottom() - 12.0));
            let mut bui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(br)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            if bui.add(brand_button("CLOSE", theme::MUTED)).clicked() {
                std::process::exit(if failed { 1 } else { 0 });
            }
            if done && self.launch {
                // When "Launch on finish" is set, the update loop auto-launches
                // after a brief beat (no click needed). This button is the manual
                // "launch right now" affordance for that short window.
                if bui.add(brand_button("LAUNCH  ▸", v)).clicked() {
                    self.launched = true;
                    self.launch_and_exit();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// painting helpers (creative HUD motifs)
// ---------------------------------------------------------------------------

/// Drifting Akira/retro-futurism perspective floor-grid in the lower void.
fn paint_grid(p: &egui::Painter, rect: Rect, t: f64) {
    let v = theme::voice();
    let col = Color32::from_rgba_unmultiplied(v.r(), v.g(), v.b(), 16);
    let vp = Pos2::new(rect.center().x, rect.top() + rect.height() * 0.36);
    let base = rect.bottom();
    // converging verticals
    let n = 14;
    for i in -n..=n {
        let fx = rect.center().x + i as f32 * (rect.width() / (n as f32 * 1.4));
        p.line_segment([vp, Pos2::new(fx, base)], Stroke::new(1.0, col));
    }
    // horizontals drifting toward the viewer
    let drift = (t * 0.25).fract() as f32;
    for k in 0..12 {
        let f = (k as f32 + drift) / 12.0;
        let y = vp.y + (base - vp.y) * f * f; // perspective easing
        let a = (18.0 * (1.0 - f)) as u8;
        p.hline(rect.x_range(), y, Stroke::new(1.0, Color32::from_rgba_unmultiplied(v.r(), v.g(), v.b(), a)));
    }
}

/// Eva-style circular sync gauge (segmented ring filling to `frac`).
fn paint_gauge(p: &egui::Painter, c: Pos2, r: f32, frac: f32, v: Color32, t: f64, failed: bool) {
    let segs = 64;
    let fill = (segs as f32 * frac).round() as i32;
    let spin = (t * 0.5) as f32;
    for i in 0..segs {
        let a = std::f32::consts::TAU * (i as f32 / segs as f32) - std::f32::consts::FRAC_PI_2;
        let p0 = Pos2::new(c.x + a.cos() * (r - 7.0), c.y + a.sin() * (r - 7.0));
        let p1 = Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r);
        let on = i < fill;
        let col = if failed {
            theme::RED
        } else if on {
            v
        } else {
            theme::dimmed(v, 0.8)
        };
        p.line_segment([p0, p1], Stroke::new(2.2, col));
    }
    // outer + inner hairline rings + a rotating tick (mecha telemetry)
    p.circle_stroke(c, r + 4.0, Stroke::new(1.0, theme::HAIRLINE));
    p.circle_stroke(c, r - 14.0, Stroke::new(1.0, theme::dimmed(v, 0.6)));
    if !failed {
        let a = spin.rem_euclid(std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2;
        let tp = Pos2::new(c.x + a.cos() * (r + 9.0), c.y + a.sin() * (r + 9.0));
        p.circle_filled(tp, 2.0, v);
    }
}

/// The NERV-style ITASHA.CORP wordmark (wide tracking; serif unavailable → mono
/// with generous letter-spacing reads as the brand mark).
fn wordmark(p: &egui::Painter, center: Pos2) {
    p.text(
        center,
        Align2::CENTER_CENTER,
        "I T A S H A . C O R P",
        FontId::monospace(13.0),
        theme::MUTED,
    );
}

fn paint_strip(p: &egui::Painter, rect: Rect) {
    let y = rect.bottom() - 20.0;
    p.hline(rect.x_range(), y - 8.0, Stroke::new(1.0, theme::HAIRLINE));
    p.text(Pos2::new(rect.left() + 24.0, y), Align2::LEFT_CENTER, "ITASHA.CORP", FontId::monospace(9.5), theme::STRIP);
    p.text(rect.center_bottom() - Vec2::new(0.0, 12.0), Align2::CENTER_CENTER, format!("{} · present day · present time", config::APP_NAME), FontId::monospace(9.5), theme::STRIP);
    p.text(Pos2::new(rect.right() - 24.0, y), Align2::RIGHT_CENTER, "F0RG3-W1R3 · CRT-IC v2", FontId::monospace(9.5), theme::STRIP);
}

fn section_label(ui: &mut egui::Ui, s: &str) {
    ui.label(egui::RichText::new(s).color(theme::voice()).monospace().strong());
}

fn opt_row(ui: &mut egui::Ui, val: &mut bool, label: impl Into<String>, _v: Color32) {
    ui.horizontal(|ui| {
        ui.checkbox(val, "");
        ui.label(egui::RichText::new(label.into()).color(theme::TEXT));
    });
    ui.add_space(2.0);
}

fn brand_button(text: &str, accent: Color32) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_string()).color(theme::TEXT).monospace())
        .stroke(Stroke::new(1.2, accent))
        .fill(theme::dimmed(accent, 0.82))
}

fn wrap(s: &str, w: usize) -> String {
    if s.len() <= w {
        return s.to_string();
    }
    let mut out = String::new();
    let mut line = 0;
    for word in s.split_whitespace() {
        if line + word.len() + 1 > w {
            out.push('\n');
            line = 0;
        }
        if line > 0 {
            out.push(' ');
            line += 1;
        }
        out.push_str(word);
        line += word.len();
    }
    out
}

/// Best-effort folder picker via PowerShell (no extra crate).
fn pick_folder() -> Option<String> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; \
        $f=New-Object System.Windows.Forms.FolderBrowserDialog; \
        if($f.ShowDialog() -eq 'OK'){ $f.SelectedPath }";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", script])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Headless install (winget/enterprise unattended + automated testing).
/// Flags: --silent [--dir <path>] [--no-start-menu] [--desktop] [--add-path]
fn silent_install(args: &[String]) -> ! {
    let flag = |n: &str| args.iter().any(|a| a == n);
    let val = |n: &str| {
        args.iter()
            .position(|a| a == n)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let dir = val("--dir").unwrap_or_else(default_dir);
    let opts = engine::Opts {
        dir: PathBuf::from(dir.trim().trim_matches('"')),
        start_menu: !flag("--no-start-menu"),
        desktop: flag("--desktop"),
        add_path: flag("--add-path"),
    };
    println!("itasha-installer: silent install -> {}", opts.dir.display());
    let cb = |s: engine::Step| println!("  [{:>3.0}%] {}", s.frac * 100.0, s.label);
    match engine::install(&opts, PAYLOAD, &cb) {
        Ok(()) => {
            println!("OK: {} installed.", config::APP_NAME);
            std::process::exit(0)
        }
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::exit(1)
        }
    }
}

/// A small CRT-styled, voice-coloured window/taskbar icon (generated, no asset).
fn app_icon() -> egui::IconData {
    let (w, h) = (64usize, 64usize);
    let v = theme::voice();
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let (mut r, mut g, mut b) = (7u8, 10u8, 12u8); // void
            let inset = 8;
            let inside = x > inset && x < w - 1 - inset && y > inset && y < h - 1 - inset;
            let border = (x == inset || x == w - 1 - inset || y == inset || y == h - 1 - inset)
                && x >= inset
                && x <= w - 1 - inset
                && y >= inset
                && y <= h - 1 - inset;
            if inside {
                r = 12;
                g = 16;
                b = 19;
            }
            if border {
                r = v.r();
                g = v.g();
                b = v.b();
            }
            let (dx, dy) = (x as i32 - 32, y as i32 - 32);
            if dx * dx + dy * dy < 18 {
                r = v.r();
                g = v.g();
                b = v.b();
            }
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = 255;
        }
    }
    egui::IconData {
        rgba,
        width: w as u32,
        height: h as u32,
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--uninstall") {
        let _ = engine::uninstall();
        return Ok(());
    }
    if args.iter().any(|a| a == "--silent") {
        silent_install(&args);
    }

    let options = eframe::NativeOptions {
        // NOTE: we deliberately do NOT set `centered: true`. eframe's builder-time
        // centering mis-places the window on Windows multi-monitor / mixed-DPI
        // setups (the window↔monitor DPI relationship is not resolved until after
        // creation — emilk/egui #4918), which is exactly the "installer not
        // centred" report. Instead the window is created HIDDEN and centred +
        // revealed on the first frame the monitor geometry is known, in
        // `App::center_and_reveal` — accurate, DPI-correct, and flash-free.
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 560.0])
            .with_min_inner_size([900.0, 560.0])
            .with_resizable(false)
            .with_visible(false) // shown by center_and_reveal once placed
            .with_icon(std::sync::Arc::new(app_icon()))
            .with_title(format!("{} — Itasha.Corp installer", config::APP_NAME)),
        ..Default::default()
    };
    eframe::run_native(
        "itasha-installer",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::<App>::default())
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::centered_outer_position;
    use eframe::egui::{pos2, vec2};

    #[test]
    fn centers_window_on_monitor() {
        // 900x560 window on a 1920x1080 monitor → ((1920-900)/2, (1080-560)/2).
        assert_eq!(
            centered_outer_position(vec2(1920.0, 1080.0), vec2(900.0, 560.0)),
            pos2(510.0, 260.0)
        );
    }

    #[test]
    fn centers_on_a_hidpi_monitor() {
        // A 3840x2160 monitor reported in points after 2x scaling is 1920x1080;
        // the math is in points so it stays correct regardless of DPI.
        assert_eq!(
            centered_outer_position(vec2(2560.0, 1440.0), vec2(900.0, 560.0)),
            pos2(830.0, 440.0)
        );
    }

    #[test]
    fn clamps_to_origin_when_window_exceeds_monitor() {
        // Never position partly off-screen: a window wider/taller than the
        // monitor lands at the origin on the offending axis.
        assert_eq!(
            centered_outer_position(vec2(800.0, 600.0), vec2(900.0, 560.0)),
            pos2(0.0, 20.0)
        );
        assert_eq!(
            centered_outer_position(vec2(700.0, 500.0), vec2(900.0, 560.0)),
            pos2(0.0, 0.0)
        );
    }
}
