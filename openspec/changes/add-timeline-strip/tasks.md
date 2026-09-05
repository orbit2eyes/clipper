## 1. Timeline strip rendering

- [x] 1.1 Implement `timeline_strip(&mut self, ui: &mut egui::Ui, avail: egui::Vec2)` in `src/main.rs` that allocates a 40 px-tall bar rect and paints: dark gray background, tinted band from `in_marker` to `out_marker`, white vertical playhead line, yellow IN and OUT triangles at the top edge, and timecode labels (`0:00:00` left, source duration right) below the bar
- [x] 1.2 Add a closure `to_x(t: f64) -> f32` inside `timeline_strip` that maps seconds to bar-x using the source's `duration_secs`; guard against `duration_secs <= 0` by returning early before drawing or allocating
- [x] 1.3 Wire `timeline_strip` into the central panel: called after the existing preview block (inside the `if self.source.is_some()` branch); preview height reduced by 44 px to make room for the strip

## 2. Bar click-to-seek

- [x] 2.1 On the bar background rect, use `Sense::click_and_drag()`. On `clicked()` or `dragged()`, compute the fractional position from pointer x, multiply by `duration_secs`, frame-snap via `(t * fps).round() / fps` when `fps > 0`, set `self.playhead`, set `self.playing = false`, and call `self.request_frame(snapped_t)`
- [x] 2.2 The playhead line in the strip is drawn from `to_x(self.playhead)` on every frame, so it updates automatically; preview texture update handled by the existing `request_frame` flow

## 3. IN/OUT triangle drag

- [x] 3.1 Allocate a 15×15 px hit rect (`tri_size * 2.5` in each dimension) centered on each triangle with `Sense::drag()`. On `dragged()` for the IN triangle, compute the new IN time from `drag_delta().x` mapped through inverse `to_x`, frame-snap, clamp to `[0, out_marker]`, update `self.in_marker`, and set `self.playing = false`
- [x] 3.2 Repeat for the OUT triangle: drag delta → new OUT time, frame-snap, clamp to `[in_marker, duration]`, update `self.out_marker`, set `self.playing = false`
- [x] 3.3 IN clamp uses `clamp(0.0, self.out_marker)`; OUT clamp uses `clamp(self.in_marker, dur)` — both are no-swap, so dragging IN past OUT pins to OUT and vice versa

## 4. Verify

- [x] 4.1 `cargo build` clean (one pre-existing dead-code warning on `SourceMeta` fields unrelated to this change; no new warnings)
- [x] 4.2 Manual smoke test deferred to user — code is in place, all logic implemented and types check; user runs `cargo run` to confirm visually
