## Why

clipper currently exports one Shorts clip per Export click: load, set IN/OUT, click Export, repeat. A user producing several Shorts from one source (the typical podcast-with-clips workflow) has to manually repeat that loop. This change adds a clip queue so the user can build up several IN/OUT pairs and export them all in one batch, with one cancellation that aborts the whole run.

## What Changes

- Add a clip queue: an ordered list of `(in_marker, out_marker)` pairs that the user accumulates while scrubbing the timeline.
- New right-side panel in the main window showing the queue as a scrollable list of rows, each row showing the clip number, IN time, OUT time, and a delete (`×`) button.
- Add an "Add at IN/OUT" button that pushes the current `in_marker` and `out_marker` into the queue.
- Add an "Export all" button that runs the export pipeline once per queued clip, sequentially, writing `short_NNN.mp4` per clip with auto-incremented numbers.
- Reuse the existing crop and source-resolution settings across all clips in the queue (shared crop, single source).
- Progress display during a batch shows current clip index and per-clip percentage; the transport bar's existing export-progress affordance is reused.
- Add a cancel action for an in-progress batch: kills the current ffmpeg process, removes any partial output, and discards the remaining queue.
- The existing single-clip "Export" button stays: pressing it while the queue is empty exports the current IN/OUT one-off, identical to current behavior.
- Adding a clip with `IN >= OUT` is silently ignored (matches existing IN/OUT validation).
- The "+ Add at IN/OUT" and "Export all" buttons are disabled while a batch is running, to keep the export pipeline's state machine simple.

## Capabilities

### New Capabilities

None. The clip queue is part of the export workflow and stays inside the existing `clip-export` capability.

### Modified Capabilities

- `clip-export`: add requirements describing the clip queue (add at IN/OUT, list rendering, remove by `×`, sequential batch export, per-clip progress, cancel mid-batch, auto-numbering across the batch, disabled buttons during a running batch).

## Impact

- New state on `ClipperApp`: `clips: Vec<(f64, f64)>`, `export_queue: Option<VecDeque<(f64, f64)>>`, `export_index: usize`, plus a "batch in progress" flag derived from `export_queue.is_some()`.
- New UI: a right-side `egui::SidePanel::right("clip_queue")` showing the queue rows, an "Add at IN/OUT" button, and an "Export all" button.
- Modified export state machine: when a batch is running, finishing one clip's ffmpeg process triggers the next clip from `export_queue` until the queue drains.
- No new dependencies. No changes to `ffmpeg.rs`. Crop math, ffmpeg filter, and `next_short_name` are unchanged.
- ~200 lines of new code in `src/main.rs`; one new file `src/main.rs` import for `std::collections::VecDeque`.
- Behavior of the single-clip "Export" button is preserved when the queue is empty.
