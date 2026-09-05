mod ffmpeg;

use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("clipper")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "clipper",
        options,
        Box::new(|_cc| Ok(Box::new(ClipperApp::default()))),
    )
}

enum FrameMsg {
    Image(image::DynamicImage, f64, Option<String>),
    Done,
}

#[derive(Default)]
struct ClipperApp {
    source: Option<ffmpeg::SourceMeta>,
    error: Option<String>,
    playhead: f64,
    in_marker: f64,
    out_marker: f64,
    preview: PreviewState,
    playing: bool,
    crop: Option<CropRect>,
    export: ExportState,
    output_dir: Option<PathBuf>,
    last_export: Option<PathBuf>,
}

#[derive(Default)]
struct ExportState {
    child: Option<std::process::Child>,
    progress_rx: Option<Receiver<ExportMsg>>,
    progress: f32,
    in_progress: bool,
    error: Option<String>,
}

enum ExportMsg {
    Progress(f32),
    Done(Result<PathBuf, String>),
}

#[derive(Default)]
struct PreviewState {
    texture: Option<egui::TextureHandle>,
    pending: Option<Receiver<FrameMsg>>,
    last_loaded: f64,
}

#[derive(Clone, Copy, Debug)]
struct CropRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

impl CropRect {
    fn default_for(src_w: u32, src_h: u32) -> Self {
        let h = src_h;
        let w = (src_h as f64 * 9.0 / 16.0).round() as u32;
        let x = ((src_w as i32 - w as i32) / 2).max(0);
        let y = 0;
        CropRect { x, y, w, h }
    }

    fn clamp_to(&mut self, src_w: u32, src_h: u32) {
        self.w = self.w.min(src_w).max(1);
        self.h = self.h.min(src_h).max(1);
        if self.x < 0 { self.x = 0; }
        if self.y < 0 { self.y = 0; }
        if (self.x as u32 + self.w) > src_w {
            self.x = (src_w - self.w) as i32;
        }
        if (self.y as u32 + self.h) > src_h {
            self.y = (src_h - self.h) as i32;
        }
    }

}

impl ClipperApp {
    fn load_path(&mut self, path: PathBuf) {
        self.error = None;
        self.preview = PreviewState::default();
        self.playhead = 0.0;
        self.playing = false;
        match ffmpeg::probe(&path) {
            Ok(meta) => {
                self.in_marker = 0.0;
                self.out_marker = meta.duration_secs;
                self.crop = Some(CropRect::default_for(meta.width, meta.height));
                self.source = Some(meta);
            }
            Err(e) => self.error = Some(format!("Could not load {}: {}", path.display(), e)),
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Video", &["mp4", "mov", "mkv", "webm", "avi", "m4v", "flv"])
            .pick_file()
        {
            self.load_path(p);
        }
    }

    fn request_frame(&mut self, t: f64) {
        let Some(meta) = &self.source.clone() else { return };
        let t = t.clamp(0.0, meta.duration_secs);
        if (t - self.preview.last_loaded).abs() < 0.001 && self.preview.texture.is_some() {
            return;
        }
        let path = meta.path.clone();
        let sw = meta.width;
        let sh = meta.height;
        let (tx, rx) = channel();
        self.preview.pending = Some(rx);
        self.preview.last_loaded = t;
        std::thread::spawn(move || {
            match ffmpeg::extract_frame(&path, t, sw, sh) {
                Ok(img) => { let _ = tx.send(FrameMsg::Image(img, t, None)); }
                Err(e) => { let _ = tx.send(FrameMsg::Done); let _ = e; }
            }
        });
    }

    fn poll_frame(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.preview.pending.take() else { return };
        match rx.try_recv() {
            Ok(FrameMsg::Image(img, t, ffmpeg_msg)) => {
                if let Some(m) = ffmpeg_msg {
                    if self.error.as_deref() != Some(m.as_str()) {
                        self.error = Some(m);
                    }
                }
                self.playhead = t;
                let size = [img.width() as usize, img.height() as usize];
                let rgba = img.to_rgba8();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                let handle = ctx.load_texture("preview", color_image, egui::TextureOptions::LINEAR);
                self.preview.texture = Some(handle);
                // Keep the receiver so we can read more frames next update.
                self.preview.pending = Some(rx);
            }
            Ok(FrameMsg::Done) => {
                self.playing = false;
                // No more frames coming; leave pending cleared.
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.preview.pending = Some(rx);
                ctx.request_repaint();
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.playing = false;
                self.error = Some(self.error.take().unwrap_or_else(|| "playback ended".to_string()));
            }
        }
    }

    fn toggle_play(&mut self) {
        eprintln!("[clipper] toggle_play: playing={} source_loaded={} IN={:.2} OUT={:.2} playhead={:.2}",
            self.playing, self.source.is_some(), self.in_marker, self.out_marker, self.playhead);
        if self.playing {
            self.playing = false;
            return;
        }
        let Some(meta) = self.source.clone() else {
            self.error = Some("Open a video first".to_string());
            return;
        };
        if self.out_marker <= self.in_marker {
            self.error = Some("Cannot play: IN must be less than OUT".to_string());
            return;
        }
        let from = self.playhead.max(self.in_marker);
        let to = self.out_marker;
        if to <= from {
            self.error = Some(format!(
                "Cannot play: playhead ({:.2}s) is past OUT ({:.2}s)",
                self.playhead, self.out_marker
            ));
            return;
        }
        self.spawn_play(meta.path, meta.width, meta.height, meta.fps, from, to);
    }

    fn spawn_play(&mut self, path: PathBuf, width: u32, height: u32, fps: f64, from: f64, to: f64) {
        let frame_rx = match ffmpeg::play_stream(path.clone(), from, to, width, height, fps) {
            Ok(rx) => {
                eprintln!("[clipper] play_stream spawned for {} ({}x{}, {:.2}s → {:.2}s)",
                    path.display(), width, height, from, to);
                rx
            }
            Err(e) => {
                eprintln!("[clipper] play_stream FAILED to spawn: {}", e);
                self.error = Some(format!("Could not start playback: {}", e));
                return;
            }
        };

        // Bridge the (image, ffmpeg_stderr, frame_time) stream into our FrameMsg stream.
        let (tx, rx) = channel::<FrameMsg>();
        std::thread::spawn(move || {
            while let Ok((img, ffmpeg_msg, frame_time)) = frame_rx.recv() {
                let msg = match ffmpeg_msg {
                    Some(m) => Some(format!("ffmpeg: {}", m)),
                    None => None,
                };
                if tx.send(FrameMsg::Image(img, frame_time, msg)).is_err() { break; }
            }
            let _ = tx.send(FrameMsg::Done);
        });

        self.playing = true;
        self.preview.pending = Some(rx);
    }

    fn start_export(&mut self) {
        if self.export.in_progress { return; }
        let (Some(meta), Some(crop)) = (self.source.clone(), self.crop) else { return };
        if self.out_marker <= self.in_marker {
            self.error = Some("OUT must be greater than IN".to_string());
            return;
        }
        let dir = match &self.output_dir {
            Some(d) => d.clone(),
            None => match rfd::FileDialog::new().pick_folder() {
                Some(d) => { self.output_dir = Some(d.clone()); d }
                None => return,
            },
        };
        let output = ffmpeg::next_short_name(&dir);
        let req = ffmpeg::ExportRequest {
            source: meta.path.clone(),
            in_seconds: self.in_marker,
            out_seconds: self.out_marker,
            crop: (crop.x, crop.y, crop.w, crop.h),
            output: output.clone(),
            has_audio: meta.audio_codec.is_some(),
        };

        let mut child = ffmpeg::spawn_export(req);
        let stdout = child.stdout.take().expect("stdout piped");
        let (tx, rx) = channel();
        self.export.progress_rx = Some(rx);
        self.export.in_progress = true;
        self.export.progress = 0.0;
        self.export.error = None;
        self.export.child = Some(child);

        let duration = (self.out_marker - self.in_marker).max(0.001);
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("out_time_ms=") {
                    if let Ok(v) = rest.trim().parse::<f64>() {
                        let p = (v / 1_000_000.0 / duration).min(1.0) as f32;
                        let _ = tx.send(ExportMsg::Progress(p));
                    }
                } else if line.starts_with("progress=end") {
                    break;
                }
            }
            let _ = tx.send(ExportMsg::Done(Ok(output)));
        });
    }

    fn poll_export(&mut self) {
        let Some(rx) = self.export.progress_rx.take() else { return };
        loop {
            match rx.try_recv() {
                Ok(ExportMsg::Progress(p)) => {
                    self.export.progress = p;
                    self.export.progress_rx = Some(rx);
                    return;
                }
                Ok(ExportMsg::Done(result)) => {
                    if let Some(mut child) = self.export.child.take() {
                        let _ = child.wait();
                    }
                    self.export.in_progress = false;
                    self.export.progress = 1.0;
                    match result {
                        Ok(path) => {
                            self.last_export = Some(path);
                            self.error = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.export.progress_rx = Some(rx);
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some(mut child) = self.export.child.take() {
                        let _ = child.wait();
                    }
                    self.export.in_progress = false;
                    return;
                }
            }
        }
    }

    fn cancel_export(&mut self) {
        if let Some(mut child) = self.export.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // remove partial output file if any
        if let Some(rx) = self.export.progress_rx.take() {
            // drain
            while rx.try_recv().is_ok() {}
        }
        self.export.in_progress = false;
        self.export.progress = 0.0;
    }

    fn timeline_strip(&mut self, ui: &mut egui::Ui, avail: egui::Vec2) {
        let Some(meta) = self.source.clone() else { return };
        let dur = meta.duration_secs;
        if dur <= 0.0 { return; }
        let fps = meta.fps;

        let strip_h = avail.y.min(40.0);
        let (bar_rect, bar_resp) = ui.allocate_exact_size(
            egui::vec2(avail.x, strip_h),
            egui::Sense::click_and_drag(),
        );

        let to_x = |t: f64| -> f32 {
            let frac = (t / dur).clamp(0.0, 1.0) as f32;
            bar_rect.left() + frac * bar_rect.width()
        };

        // Background
        ui.painter().rect_filled(bar_rect, 2.0, egui::Color32::from_gray(35));

        // Cut region band
        let in_x = to_x(self.in_marker);
        let out_x = to_x(self.out_marker);
        if in_x < out_x {
            let band = egui::Rect::from_min_max(
                egui::pos2(in_x, bar_rect.top()),
                egui::pos2(out_x, bar_rect.bottom()),
            );
            ui.painter().rect_filled(band, 0.0, egui::Color32::from_rgb(60, 100, 160));
        }

        // Playhead line
        let play_x = to_x(self.playhead);
        ui.painter().line_segment(
            [egui::pos2(play_x, bar_rect.top()), egui::pos2(play_x, bar_rect.bottom())],
            egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
        );

        // IN / OUT triangles
        let tri_size = 6.0_f32;
        let in_pos = egui::pos2(in_x, bar_rect.top());
        let out_pos = egui::pos2(out_x, bar_rect.top());
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                in_pos + egui::vec2(-tri_size, 0.0),
                in_pos + egui::vec2(tri_size, 0.0),
                in_pos + egui::vec2(0.0, tri_size * 1.5),
            ],
            egui::Color32::YELLOW,
            egui::Stroke::NONE,
        ));
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                out_pos + egui::vec2(-tri_size, 0.0),
                out_pos + egui::vec2(tri_size, 0.0),
                out_pos + egui::vec2(0.0, tri_size * 1.5),
            ],
            egui::Color32::YELLOW,
            egui::Stroke::NONE,
        ));

        // Edge time labels (HH:MM:SS)
        let label_fmt = |t: f64| -> String {
            let total = t as u64;
            let h = total / 3600;
            let m = (total % 3600) / 60;
            let s = total % 60;
            format!("{:01}:{:02}:{:02}", h, m, s)
        };
        ui.painter().text(
            bar_rect.left_bottom() + egui::vec2(4.0, -2.0),
            egui::Align2::LEFT_BOTTOM,
            label_fmt(0.0),
            egui::FontId::monospace(10.0),
            egui::Color32::from_gray(180),
        );
        ui.painter().text(
            bar_rect.right_bottom() + egui::vec2(-4.0, -2.0),
            egui::Align2::RIGHT_BOTTOM,
            label_fmt(dur),
            egui::FontId::monospace(10.0),
            egui::Color32::from_gray(180),
        );

        // Click-to-seek-and-play on bar background: clicking anywhere in
        // [IN, OUT] starts playback from the clicked time and plays to OUT.
        // Clicks outside that range fall back to seeking only (no play).
        if bar_resp.clicked() || bar_resp.dragged() {
            if let Some(pos) = bar_resp.interact_pointer_pos() {
                let frac = ((pos.x - bar_rect.left()) / bar_rect.width()).clamp(0.0, 1.0) as f64;
                let raw_t = frac * dur;
                let snapped_t = if fps > 0.0 { (raw_t * fps).round() / fps } else { raw_t };
                let clamped_t = snapped_t.clamp(self.in_marker, self.out_marker);
                self.playhead = clamped_t;
                self.request_frame(clamped_t);

                // Auto-play: from click (clamped to [IN, OUT]) to OUT.
                if self.out_marker > self.in_marker && clamped_t < self.out_marker {
                    self.spawn_play(meta.path, meta.width, meta.height, fps, clamped_t, self.out_marker);
                }
            }
        }

        // IN triangle drag
        let in_tri_rect = egui::Rect::from_center_size(
            in_pos,
            egui::vec2(tri_size * 2.5, tri_size * 2.5),
        );
        let in_tri_resp = ui.allocate_rect(in_tri_rect, egui::Sense::drag());
        if in_tri_resp.dragged() {
            let delta_x = in_tri_resp.drag_delta().x;
            let delta_t = (delta_x / bar_rect.width() as f32) as f64 * dur;
            let raw = self.in_marker + delta_t;
            let snapped = if fps > 0.0 { (raw * fps).round() / fps } else { raw };
            let new_in = snapped.clamp(0.0, self.out_marker);
            if (new_in - self.in_marker).abs() > 1e-6 {
                self.in_marker = new_in;
                self.playing = false;
            }
        }

        // OUT triangle drag
        let out_tri_rect = egui::Rect::from_center_size(
            out_pos,
            egui::vec2(tri_size * 2.5, tri_size * 2.5),
        );
        let out_tri_resp = ui.allocate_rect(out_tri_rect, egui::Sense::drag());
        if out_tri_resp.dragged() {
            let delta_x = out_tri_resp.drag_delta().x;
            let delta_t = (delta_x / bar_rect.width() as f32) as f64 * dur;
            let raw = self.out_marker + delta_t;
            let snapped = if fps > 0.0 { (raw * fps).round() / fps } else { raw };
            let new_out = snapped.clamp(self.in_marker, dur);
            if (new_out - self.out_marker).abs() > 1e-6 {
                self.out_marker = new_out;
                self.playing = false;
            }
        }
    }
}

impl eframe::App for ClipperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drag-and-drop
        let dropped: Vec<PathBuf> = ctx
            .input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.load_path(p);
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open…").clicked() { self.open_file_dialog(); }
            });
        });

        egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let play_label = if self.playing { "Stop" } else { "Play" };
                if ui.button(play_label).clicked() { self.toggle_play(); }
                if let Some(meta) = &self.source {
                    ui.label(format!("{:.2}s / {:.2}s", self.playhead, meta.duration_secs));
                    if ui.button("Set IN").clicked() { self.in_marker = self.playhead; }
                    if ui.button("Set OUT").clicked() { self.out_marker = self.playhead; }
                    ui.label(format!("IN {:.2}  OUT {:.2}", self.in_marker, self.out_marker));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.export.in_progress {
                        ui.add(egui::ProgressBar::new(self.export.progress).show_percentage());
                        if ui.button("Cancel").clicked() { self.cancel_export(); }
                    } else {
                        let label = if self.last_export.is_some() { "Export next" } else { "Export" };
                        if ui.button(label).clicked() { self.start_export(); }
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(err) = self.error.clone() {
                ui.colored_label(egui::Color32::RED, err);
            }
            if let Some(last) = &self.last_export {
                ui.colored_label(egui::Color32::GREEN, format!("Saved: {}", last.display()));
            }

            if self.source.is_some() {
                self.poll_frame(ctx);
                self.poll_export();

                let full_avail = ui.available_size();
                let avail = egui::vec2(full_avail.x, (full_avail.y - 44.0).max(100.0));
                let resp = ui.allocate_response(avail, egui::Sense::click_and_drag());

                if let (Some(tex), Some(meta_clone)) = (&self.preview.texture, self.source.clone()) {
                    let meta = &meta_clone;
                    let tex_size = tex.size_vec2();
                    let scale = (avail.x / tex_size.x).min(avail.y / tex_size.y);
                    let display = tex_size * scale;
                    let display_rect = egui::Rect::from_center_size(resp.rect.center(), display);

                    // source-frame-to-display-scale and display-to-source
                    let src_to_disp = display.x / meta.width as f32;
                    let disp_to_src = |v: f32| (v / src_to_disp) as i32;

                    ui.painter().image(
                        tex.id(),
                        display_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );

                    // crop overlay
                    if let Some(crop) = &self.crop {
                        let crop_screen = egui::Rect::from_min_size(
                            display_rect.min
                                + egui::vec2(crop.x as f32 * src_to_disp, crop.y as f32 * src_to_disp),
                            egui::vec2(crop.w as f32 * src_to_disp, crop.h as f32 * src_to_disp),
                        );
                        ui.painter().rect_stroke(
                            crop_screen,
                            0.0,
                            egui::Stroke::new(2.0_f32, egui::Color32::YELLOW),
                            egui::StrokeKind::Inside,
                        );
                        // shade outside the crop
                        let dark = |rect: egui::Rect| {
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                egui::Color32::from_black_alpha(120),
                            );
                        };
                        dark(egui::Rect::from_min_max(display_rect.min, egui::pos2(crop_screen.max.x, crop_screen.min.y)));
                        dark(egui::Rect::from_min_max(egui::pos2(crop_screen.max.x, display_rect.min.y), egui::pos2(display_rect.max.x, crop_screen.min.y)));
                        dark(egui::Rect::from_min_max(egui::pos2(crop_screen.min.x, crop_screen.max.y), egui::pos2(crop_screen.max.x, display_rect.max.y)));
                        dark(egui::Rect::from_min_max(egui::pos2(display_rect.min.x, crop_screen.max.y), egui::pos2(crop_screen.min.x, display_rect.max.y)));
                    }

                    // scrub / drag
                    if resp.clicked() || resp.drag_started() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let frac = ((pos.x - display_rect.left()) / display_rect.width()).clamp(0.0, 1.0) as f64;
                            self.playhead = frac * meta.duration_secs;
                            self.request_frame(self.playhead);
                        }
                    }

                    // drag crop
                    if let Some(crop) = self.crop.clone() {
                        let crop_screen = egui::Rect::from_min_size(
                            display_rect.min
                                + egui::vec2(crop.x as f32 * src_to_disp, crop.y as f32 * src_to_disp),
                            egui::vec2(crop.w as f32 * src_to_disp, crop.h as f32 * src_to_disp),
                        );
                        let crop_resp = ui.allocate_rect(crop_screen, egui::Sense::drag());
                        if crop_resp.dragged() {
                            let delta = crop_resp.drag_delta();
                            if let Some(c) = &mut self.crop {
                                c.x += disp_to_src(delta.x);
                                c.y += disp_to_src(delta.y);
                                c.clamp_to(meta.width, meta.height);
                            }
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| ui.label("Loading preview…"));
                }
                // Timeline strip below the preview
                self.timeline_strip(ui, egui::vec2(full_avail.x, 40.0));
            } else {
                ui.heading("clipper");
                ui.label("Open a video file or drop one onto the window.");
            }
        });

        if self.playing {
            ctx.request_repaint();
        }
    }
}
