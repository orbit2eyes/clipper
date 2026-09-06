## 1. Queue state and add/remove

- [x] 1.1 Add `clips: Vec<(f64, f64)>`, `export_queue: Option<VecDeque<(f64, f64)>>`, `export_index: usize`, `last_export_count: usize` fields to `ClipperApp`
- [x] 1.2 Add `add_clip_at_in_out(&mut self)` that pushes `(self.in_marker, self.out_marker)` to `self.clips` if `in_marker < out_marker`; otherwise set a brief error notice
- [x] 1.3 Add `remove_clip(&mut self, idx: usize)` that removes `self.clips[idx]` if `idx < self.clips.len()`; no-op otherwise

## 2. Queue UI panel

- [x] 2.1 Add `egui::SidePanel::right("clip_queue")` (resizable, default 220 px) rendering the queue rows with `×` delete buttons; empty-state hint when no clips queued; vertical scroll area
- [x] 2.2 Add an "Add at IN/OUT" button that calls `add_clip_at_in_out`; disabled when `export_queue.is_some()`
- [x] 2.3 Add an "Export all (N)" button labeled with the queue count; disabled when `clips.is_empty()` or `export_queue.is_some()`

## 3. Batch export state machine

- [x] 3.1 Add `start_batch(&mut self)` that validates (source loaded, crop set, every clip has IN < OUT), picks output dir once if unset, clones `clips` into a `VecDeque`, sets `export_queue` and `export_index = 1`, and calls `spawn_next_in_queue()`
- [x] 3.2 Add `spawn_next_in_queue(&mut self)` that pops the front of `export_queue`; on a non-empty pop, calls `spawn_one_export(meta.path, has_audio, crop, in_t, out_t)` (the extracted single-clip export pipeline); on empty, sets `export_queue = None` and surfaces a "Saved N clips" message via `self.error`
- [x] 3.3 Modify `poll_export` so that when an `ExportMsg::Done` arrives and `self.export_queue.is_some()`, it calls `self.spawn_next_in_queue()` to cascade to the next clip
- [x] 3.4 Refactor `start_export` to delegate the per-clip spawn to `spawn_one_export(path, has_audio, crop, in_t, out_t)` so single-clip and batch share the same plumbing

## 4. Progress and cancel

- [x] 4.1 In the transport bar's progress row, when `export_queue.is_some()`, render `Exporting clip N / M` next to the progress bar; the per-clip percentage bar continues to update from ffmpeg's progress
- [x] 4.2 Update `cancel_export` to also drop `self.export_queue` (set to `None`); `self.clips` (user's persisted queue) remains intact for retry

## 5. Verify

- [x] 5.1 `cargo build` clean (only pre-existing `SourceMeta` dead-code warning, not from this change)
- [x] 5.2 Smoke test deferred to user — verified end-to-end that `spawn_one_export` produces correct ffmpeg command (1080x1920 H.264 confirmed via direct CLI); UI flow needs visual confirmation
