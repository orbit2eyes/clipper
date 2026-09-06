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
    current_crop: CropRect,
    selected_clip: Option<usize>,
    export: ExportState,
    output_dir: Option<PathBuf>,
    last_export: Option<PathBuf>,
    clips: Vec<QueuedClip>,
    export_queue: Option<std::collections::VecDeque<QueuedClip>>,
    export_index: usize,
    last_export_count: usize,
    drag_start_x: Option<f32>,
    drag_target_clip: Option<usize>,
    drag_start_clip_in: f64,
    drag_total: f32,
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

#[derive(Clone, Copy, Debug, Default)]
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

#[derive(Clone, Debug)]
struct QueuedClip {
    in_t: f64,
    out_t: f64,
    crop: CropRect,
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
                self.current_crop = CropRect::default_for(meta.width, meta.height);
                self.selected_clip = None;
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

    /// Spawn the ffmpeg child for a single clip export. Reads source/crop from
    /// `self` at call time, computes the next output filename, drives progress.
    /// Caller is responsible for setting `self.export.in_progress` and queue state.
    fn spawn_one_export(&mut self, source: PathBuf, has_audio: bool, crop: CropRect, in_t: f64, out_t: f64) {
        let Some(dir) = self.output_dir.clone() else {
            self.error = Some("Output directory not set".to_string());
            return;
        };
        let output = ffmpeg::next_short_name(&dir);
        let req = ffmpeg::ExportRequest {
            source: source.clone(),
            in_seconds: in_t,
            out_seconds: out_t,
            crop: (crop.x, crop.y, crop.w, crop.h),
            output: output.clone(),
            has_audio,
        };

        let mut child = ffmpeg::spawn_export(req);
        let stdout = child.stdout.take().expect("stdout piped");
        let (tx, rx) = channel();
        self.export.progress_rx = Some(rx);
        self.export.in_progress = true;
        self.export.progress = 0.0;
        self.export.error = None;
        self.export.child = Some(child);

        let duration = (out_t - in_t).max(0.001);
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
                    // If a batch is in progress, kick off the next clip.
                    // spawn_next_in_queue handles the empty-queue case (sets the
                    // "Saved N clips" message itself), so we just call and return.
                    if self.export_queue.is_some() {
                        self.spawn_next_in_queue();
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
        // Drop any remaining batch queue (user's persisted queue stays intact).
        self.export_queue = None;
        self.export_index = 0;
        self.export.in_progress = false;
        self.export.progress = 0.0;
    }

    fn remove_clip(&mut self, idx: usize) {
        if idx < self.clips.len() {
            self.clips.remove(idx);
        }
    }

    fn start_batch(&mut self) {
        if self.clips.is_empty() || self.export_queue.is_some() {
            return;
        }
        if self.source.is_none() {
            self.error = Some("Open a video first".to_string());
            return;
        }
        // Validate every clip's IN<OUT before we start.
        for c in &self.clips {
            if c.in_t >= c.out_t {
                self.error = Some("Cannot start batch: every clip must have IN < OUT".to_string());
                return;
            }
        }
        // Pick output dir if not set (once for the whole batch).
        if self.output_dir.is_none() {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                self.output_dir = Some(d);
            } else {
                return;
            }
        }
        let queue: std::collections::VecDeque<QueuedClip> = self.clips.iter().cloned().collect();
        let total = queue.len();
        self.export_queue = Some(queue);
        self.export_index = 1;
        self.last_export_count = total;
        self.spawn_next_in_queue();
    }

    fn spawn_next_in_queue(&mut self) {
        let Some(queue) = self.export_queue.as_mut() else { return };
        let Some(clip) = queue.pop_front() else {
            let n = self.last_export_count;
            self.export_queue = None;
            self.export_index = 0;
            self.error = Some(format!("Saved {} clips", n));
            return;
        };
        let Some(meta) = self.source.clone() else {
            self.export_queue = None;
            self.export_index = 0;
            self.error = Some("Source no longer available".to_string());
            return;
        };
        if !self.export.in_progress {
            self.spawn_one_export(meta.path, meta.audio_codec.is_some(), clip.crop, clip.in_t, clip.out_t);
            self.export_index += 1;
        }
    }

    fn timeline_strip(&mut self, ui: &mut egui::Ui, avail: egui::Vec2) {
        let Some(meta) = self.source.clone() else { return };
        let dur = meta.duration_secs;
        if dur <= 0.0 { return; }

        let strip_h = avail.y.min(40.0);
        let (bar_rect, bar_resp) = ui.allocate_exact_size(
            egui::vec2(avail.x, strip_h),
            egui::Sense::click_and_drag(),
        );

        let to_x = |t: f64| -> f32 {
            let frac = (t / dur).clamp(0.0, 1.0) as f32;
            bar_rect.left() + frac * bar_rect.width()
        };
        let x_to_t = |x: f32| -> f64 {
            let frac = ((x - bar_rect.left()) / bar_rect.width()).clamp(0.0, 1.0) as f64;
            frac * dur
        };

        // Background
        ui.painter().rect_filled(bar_rect, 2.0, egui::Color32::from_gray(35));

        // Drag-to-define a new clip: drag_start_x tracks where the drag began;
        // on drag_stopped, the range becomes a new queued clip.
        if bar_resp.drag_started() {
            if let Some(pos) = bar_resp.interact_pointer_pos() {
                self.drag_start_x = Some(pos.x);
            }
        }
        if let Some(start_x) = self.drag_start_x {
            if let Some(cur_pos) = bar_resp.interact_pointer_pos() {
                let cur_x = cur_pos.x;
                let lo = start_x.min(cur_x);
                let hi = start_x.max(cur_x);
                let drag_rect = egui::Rect::from_min_max(
                    egui::pos2(lo, bar_rect.top()),
                    egui::pos2(hi, bar_rect.bottom()),
                );
                ui.painter().rect_filled(
                    drag_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(120, 200, 130, 110),
                );
                ui.painter().rect_stroke(
                    drag_rect,
                    0.0,
                    egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(120, 200, 130)),
                    egui::StrokeKind::Inside,
                );
            }
            if bar_resp.drag_stopped() {
                if let Some(cur_pos) = bar_resp.interact_pointer_pos() {
                    let in_t = x_to_t(self.drag_start_x.unwrap_or(cur_pos.x).min(cur_pos.x));
                    let out_t = x_to_t(self.drag_start_x.unwrap_or(cur_pos.x).max(cur_pos.x));
                    if out_t - in_t >= 0.05 {
                        self.clips.push(QueuedClip {
                            in_t,
                            out_t,
                            crop: self.current_crop,
                        });
                    }
                }
                self.drag_start_x = None;
            }
        }

        // Single-click = seek (only if not currently dragging).
        if bar_resp.clicked() && self.drag_start_x.is_none() {
            if let Some(pos) = bar_resp.interact_pointer_pos() {
                let t = x_to_t(pos.x);
                self.playhead = t;
                self.request_frame(t);
            }
        }

        // Queued clips — outlined bands with a number label. Each clip is
        // interactive in three ways:
        //   - drag the body  → move the clip (width preserved)
        //   - click the body → remove the clip
        //   - drag the left edge  → resize IN (changes duration)
        //   - drag the right edge → resize OUT (changes duration)
        // Edges have higher hit-test priority than the body, so they're
        // allocated after the body rect.
        let queued_color = egui::Color32::from_rgb(120, 200, 130);
        let selected_color = egui::Color32::from_rgb(255, 220, 80);
        let to_remove: Option<usize> = None;
        let n = self.clips.len();
        for i in 0..n {
            // Copy out the values we need so we can mutate self.clips[i] later.
            let clip_in_t = self.clips[i].in_t;
            let clip_out_t = self.clips[i].out_t;
            let clip_crop = self.clips[i].crop;
            let is_selected = self.selected_clip == Some(i);
            let clip_color = if is_selected { selected_color } else { queued_color };
            let clip_outline = egui::Stroke::new(if is_selected { 3.0_f32 } else { 2.0_f32 }, clip_color);
            let ci = to_x(clip_in_t);
            let co = to_x(clip_out_t);
            if ci < co {
                let rect = egui::Rect::from_min_max(
                    egui::pos2(ci, bar_rect.top()),
                    egui::pos2(co, bar_rect.bottom()),
                );
                let edge_w = 8.0_f32;

                // Body rect (middle of clip) — drag to move; click to select
                let body_rect = egui::Rect::from_min_max(
                    egui::pos2(rect.min.x + edge_w, rect.min.y),
                    egui::pos2(rect.max.x - edge_w, rect.max.y),
                );
                // Body uses Sense::click_and_drag, but we explicitly add a
                // separate Sense::click overlay for selection so click always
                // registers regardless of egui's drag classification.
                let body_resp = ui.allocate_rect(body_rect, egui::Sense::drag());
                let body_click_resp = ui.allocate_rect(body_rect, egui::Sense::click());

                // Left edge — resize IN
                let left_edge = egui::Rect::from_min_max(
                    egui::pos2(rect.min.x, rect.min.y),
                    egui::pos2(rect.min.x + edge_w, rect.max.y),
                );
                let left_resp = ui.allocate_rect(left_edge, egui::Sense::drag());
                if left_resp.dragged() {
                    let dx = left_resp.drag_delta().x;
                    let dt = (dx / bar_rect.width() as f32) as f64 * dur;
                    let new_in = (clip_in_t + dt).clamp(0.0, clip_out_t - 0.05);
                    self.clips[i].in_t = new_in;
                }

                // Right edge — resize OUT (highest priority within clip)
                let right_edge = egui::Rect::from_min_max(
                    egui::pos2(rect.max.x - edge_w, rect.min.y),
                    egui::pos2(rect.max.x, rect.max.y),
                );
                let right_resp = ui.allocate_rect(right_edge, egui::Sense::drag());
                if right_resp.dragged() {
                    let dx = right_resp.drag_delta().x;
                    let dt = (dx / bar_rect.width() as f32) as f64 * dur;
                    let new_out = (clip_out_t + dt).clamp(clip_in_t + 0.05, dur);
                    self.clips[i].out_t = new_out;
                }

                // Draw outline + number on top of all rects
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    clip_outline,
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::monospace(11.0),
                    clip_color,
                );

                if body_resp.hovered() {
                    ui.painter().text(
                        egui::pos2(rect.center().x, bar_rect.top() - 4.0),
                        egui::Align2::CENTER_BOTTOM,
                        format!("Clip {}  (right-click to delete)", i + 1),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_gray(180),
                    );
                }

                if body_resp.drag_started() {
                    // Anchor the drag to the clip's current IN so the move
                    // is computed from drag-start position every frame.
                    self.drag_total = 0.0;
                    self.drag_target_clip = Some(i);
                    self.drag_start_clip_in = clip_in_t;
                    if let Some(pos) = body_resp.interact_pointer_pos() {
                        self.drag_start_x = Some(pos.x);
                    }
                }

                if body_resp.dragged() && self.drag_target_clip == Some(i) {
                    self.drag_total += body_resp.drag_delta().length();
                    if self.drag_total > 4.0 {
                        // Real drag — move the clip; width preserved.
                        if let (Some(start_x), Some(cur_pos)) =
                            (self.drag_start_x, body_resp.interact_pointer_pos())
                        {
                            let dx = cur_pos.x - start_x;
                            let dt = (dx / bar_rect.width() as f32) as f64 * dur;
                            let width = clip_out_t - clip_in_t;
                            let new_in = (self.drag_start_clip_in + dt).clamp(0.0, dur - width);
                            let new_out = new_in + width;
                            self.clips[i].in_t = new_in;
                            self.clips[i].out_t = new_out;
                        }
                    }
                }
                if body_resp.drag_stopped() && self.drag_target_clip == Some(i) {
                    if self.drag_total <= 4.0 {
                        // Tiny drag / click-with-jitter — treat as click → select.
                        self.selected_clip = Some(i);
                        self.current_crop = clip_crop;
                    }
                    self.drag_target_clip = None;
                    self.drag_start_x = None;
                }

                if body_resp.secondary_clicked() {
                    // Right-click — delete this clip (Premiere-style).
                    self.remove_clip(i);
                    if self.selected_clip == Some(i) {
                        self.selected_clip = None;
                    } else if let Some(sel) = self.selected_clip {
                        if sel > i {
                            self.selected_clip = Some(sel - 1);
                        }
                    }
                } else if body_click_resp.clicked() {
                    // Pure click — select this clip; preview crop jumps to its crop.
                    self.selected_clip = Some(i);
                    self.current_crop = clip_crop;
                }
            }
        }
        if let Some(i) = to_remove {
            self.remove_clip(i);
            // Clear selection if we just removed the selected clip
            if self.selected_clip == Some(i) {
                self.selected_clip = None;
            } else if let Some(sel) = self.selected_clip {
                if sel > i {
                    self.selected_clip = Some(sel - 1);
                }
            }
        }

        // Playhead line — drawn early so queued bands render on top of it.
        let play_x = to_x(self.playhead);
        ui.painter().line_segment(
            [egui::pos2(play_x, bar_rect.top()), egui::pos2(play_x, bar_rect.bottom())],
            egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
        );

        // Playhead hit rect — allocated BEFORE clips so clips win on overlap.
        // Drag here to scrub. Narrow zone (3px each side) so it only fires
        // when the user actually grabs the white line, not on a clip body.
        let playhead_hit = egui::Rect::from_min_max(
            egui::pos2(play_x - 3.0, bar_rect.top()),
            egui::pos2(play_x + 3.0, bar_rect.bottom()),
        );
        let playhead_resp = ui.allocate_rect(playhead_hit, egui::Sense::drag());
        if playhead_resp.dragged() {
            let dx = playhead_resp.drag_delta().x;
            let dt = (dx / bar_rect.width() as f32) as f64 * dur;
            let new_t = (self.playhead + dt).clamp(0.0, dur);
            self.playhead = new_t;
            self.request_frame(new_t);
        }

        // Playhead line — visual only. No interactive hit rect here because
        // it would intercept clicks on clips underneath. Seek by clicking on
        // the strip's empty area instead (handled by bar_resp.clicked).

        // Edge + tick labels in full HH:MM:SS detail.
        let label_fmt = |t: f64| -> String {
            let total = t.max(0.0) as u64;
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

        // Tick marks at adaptive intervals based on duration. Aim for
        // roughly 6–12 ticks across the strip so labels stay readable.
        let tick_secs = if dur < 30.0 {
            2.0
        } else if dur < 120.0 {
            10.0
        } else if dur < 600.0 {
            30.0
        } else if dur < 1800.0 {
            60.0
        } else if dur < 3600.0 {
            300.0
        } else {
            600.0
        };
        let mut t = tick_secs;
        while t < dur - 0.001 {
            let tx = to_x(t);
            ui.painter().line_segment(
                [
                    egui::pos2(tx, bar_rect.bottom() - 5.0),
                    egui::pos2(tx, bar_rect.bottom()),
                ],
                egui::Stroke::new(1.0_f32, egui::Color32::from_gray(140)),
            );
            ui.painter().text(
                egui::pos2(tx, bar_rect.bottom() - 7.0),
                egui::Align2::CENTER_BOTTOM,
                label_fmt(t),
                egui::FontId::monospace(9.0),
                egui::Color32::from_gray(160),
            );
            t += tick_secs;
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
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let batch_running = self.export_queue.is_some();
                    if self.export.in_progress {
                        if batch_running {
                            let total = self.last_export_count.max(1);
                            let cur = self.export_index.saturating_sub(1).min(total);
                            ui.label(format!("Exporting clip {} / {}", cur, total));
                        }
                        ui.add(egui::ProgressBar::new(self.export.progress).show_percentage());
                        if ui.button("Cancel").clicked() { self.cancel_export(); }
                    } else if batch_running {
                        let total = self.last_export_count.max(1);
                        ui.label(format!("Exporting clip {} / {}", self.export_index, total));
                    } else {
                        let export_all_enabled = !self.clips.is_empty() && !batch_running;
                        if ui
                            .add_enabled(
                                export_all_enabled,
                                egui::Button::new(format!("Export all ({})", self.clips.len())),
                            )
                            .clicked()
                        {
                            self.start_batch();
                        }
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

                    // crop overlay (uses the currently-selected crop)
                    let crop = self.current_crop;
                    {
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

                    // drag crop — only updates the SELECTED clip's crop.
                    // Drag is ignored if no clip is selected; user must
                    // click a clip first to "own" the crop edits.
                    {
                        let crop_screen = egui::Rect::from_min_size(
                            display_rect.min
                                + egui::vec2(crop.x as f32 * src_to_disp, crop.y as f32 * src_to_disp),
                            egui::vec2(crop.w as f32 * src_to_disp, crop.h as f32 * src_to_disp),
                        );
                        let crop_resp = ui.allocate_rect(crop_screen, egui::Sense::drag());
                        if crop_resp.dragged() {
                            if let Some(sel) = self.selected_clip {
                                let delta = crop_resp.drag_delta();
                                self.current_crop.x += disp_to_src(delta.x);
                                self.current_crop.y += disp_to_src(delta.y);
                                self.current_crop.clamp_to(meta.width, meta.height);
                                if let Some(c) = self.clips.get_mut(sel) {
                                    c.crop = self.current_crop;
                                }
                            }
                            // No clip selected → ignore the drag.
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
