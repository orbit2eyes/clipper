## Context

The MVP ships with single-source load, drag-crop, IN/OUT markers, real-time paced playback, a timeline strip with click-seek and triangle-drag, and a single-clip "Export" button that writes `short_NNN.mp4`. The export pipeline (in `src/ffmpeg.rs`) already takes `(in_seconds, out_seconds, crop, output)` and is fully reusable — the batch change is purely about driving the existing pipeline multiple times from user input.

Today's flow for "5 Shorts from one podcast" is: load, scrub, Set IN, Set OUT, Export, repeat. This change makes that flow: load, scrub, Set IN, Set OUT, "Add at IN/OUT", repeat 4 more times, "Export all", walk away.

## Goals / Non-Goals

**Goals:**
- Queue of `(in, out)` pairs accumulated in session; visible on the right side of the window.
- Single button to add the current IN/OUT to the queue.
- Per-row delete (`×`) for accidental additions.
- "Export all" runs the existing export pipeline sequentially for each queue entry, writing `short_NNN.mp4` files with auto-incremented numbers.
- Progress shows current clip index out of total plus per-clip percentage.
- Cancel mid-batch kills the running ffmpeg and drops remaining queue entries.
- Shared crop across all queued clips.
- Existing single-clip "Export" button keeps working when the queue is empty.

**Non-Goals:**
- Per-clip crop (all clips share one crop).
- Per-clip name overrides (auto-numbered `short_NNN.mp4`).
- Reordering the queue (drag rows, up/down buttons).
- Edit a queued clip in place (clicking a row reloads its IN/OUT into the main panel — read-only display for now).
- Batch import from CSV / external source.
- Parallel / concurrent export (sequential keeps the export state machine simple; one ffmpeg at a time is also friendlier to disk I/O).
- Persistence of the queue across sessions.

## Decisions

### D1. Right-side panel placement

`egui::SidePanel::right("clip_queue")` at default width (~200 px). The central panel (preview + timeline strip) shrinks to accommodate. The top menu and bottom transport are unaffected.

Why right and not bottom: a vertical list grows downward naturally with source duration (5–10 rows typical). Bottom would compete with the transport bar's horizontal real estate.

### D2. Queue state model

```rust
clips: Vec<(f64, f64)>,                        // user's accumulated pairs (queue)
export_queue: Option<VecDeque<(f64, f64)>>,    // remaining pairs to export; Some = batch running
export_index: usize,                          // 1-based for display
```

`export_queue` is `None` when no batch is running; `Some(deque)` when a batch is in progress. Each entry is a snapshot of `(in, out)` taken at the moment the user clicked "Export all" — so changes to `in_marker` / `out_marker` during a batch don't affect in-flight clips.

### D3. Batch export state machine

```
   click "Export all"          ffmpeg done, queue not empty
        │                              │
        ▼                              ▼
  clone clips → VecDeque     pop next (in, out)
  export_queue = Some(d)     spawn_export(in, out)
  export_index = 1           (existing pipeline)
        │                              │
        ▼                              │
  spawn_export(first clip)               │
        │                              │
        ▼                              │
  poll_export ──── done ────────────────┘
        │
        ▼ (queue drained)
  export_queue = None
  export_index = total
```

The existing `poll_export` is extended: when an `ExportMsg::Done` arrives and `export_queue.is_some()`, instead of just clearing state, it pops the next pair and spawns the next export. This keeps the state machine flat — one ffmpeg at a time, no concurrency.

### D4. Progress display

Two pieces of info during a batch:
- Per-clip percentage: the existing `egui::ProgressBar` driven by ffmpeg's `-progress pipe:1`. Unchanged.
- Batch position: a label like `Exporting clip 2 / 5` rendered next to the progress bar in the transport bar's right-aligned row.

When the batch finishes, `last_export` updates to the most recently-written path (existing behavior). Add `last_export_count: Option<usize>` for a "Saved N clips" message after the batch.

### D5. Cancel mid-batch

Reuse the existing `cancel_export()`:
- kill the ffmpeg child
- take the `export_queue` and drop it
- leave `clips` (user's queue) untouched so they can re-run

The next "Export all" click on the same `clips` re-spawns the whole queue from the top. Partial files are overwritten by the next attempt via ffmpeg's `-y` flag.

### D6. Shared crop and source

The crop rectangle and source path are read from `self.crop` and `self.source` at the moment each clip is spawned, not snapshotted at "Export all" time. This means if the user tweaks the crop during a long batch, the later clips get the new crop. Acceptable because batches are short (a few minutes total for a handful of clips), and "Export all" snapshots crop source path implicitly because there's only one source loaded at a time.

### D7. Per-row UI

```
┌─────────────────────────────┐
│  Clips                      │
├─────────────────────────────┤
│  #01  0:30 – 1:15      ×    │
│  #02  2:14 – 2:58      ×    │
│  #03  14:02 – 14:55    ×    │
├─────────────────────────────┤
│  [ + Add at IN/OUT ]        │
│  [ Export all (3) ]         │
└─────────────────────────────┘
```

The "Export all (3)" button shows the queue count. Both buttons disabled while `export_queue.is_some()`. Delete is a one-shot click on `×`; remove from `clips` immediately.

### D8. Sequential, not concurrent

Sequential export is simpler (no shared ffmpeg state, no disk I/O contention, no UI jank from N concurrent decode/encode jobs). The cost is total time = sum of clip encode times. For a typical 60-second clip at 30 fps and H.264 medium preset, ~10–15 s encode. Five clips = ~1 minute total. Acceptable.

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| User adds 50 clips, walks away, comes back to a half-finished batch | Per-clip progress + clip index gives clear "where are we" feedback. Cancel button available. |
| ffmpeg fails on clip 3 of 5; what happens? | Surface the ffmpeg error in the existing red error label, leave the rest of the queue intact, let the user fix and click "Export all" again (which will skip already-written `short_NNN.mp4` files via `next_short_name`). |
| Source path is missing (user closed the file in another app) | `spawn_export` already errors clearly; that error surfaces in the per-clip failure path. |
| Drag-drop a new file mid-batch | Disable drag-drop while `export_queue.is_some()` to avoid invalidating the in-flight export's source path. |
| Right-side panel eats horizontal preview space | Default ~200 px width; central panel shrinks accordingly. Preview already uses `avail` from the panel, so it auto-resizes. |
| Cancel leaves a `short_NNN.mp4` partial file on disk | ffmpeg's `-y` overwrites it on next attempt; partial is invisible to user because it's just the last index. Acceptable for v1. |
| Queue rows out of sync with auto-numbering | `next_short_name` scans the output dir each export, so even if user manually deletes `short_002.mp4` between batches, the next export reuses `002`. Always consistent. |

## Migration Plan

N/A — additive UI + state. Existing single-clip export path is unchanged.

## Open Questions

- Should the right-side panel be collapsible (a `>` toggle to hide the queue and give the preview more room)? Easy to add but skipped for v1.
- Should "Export all" warn if the output directory has existing `short_NNN.mp4` files that the batch would overwrite? Current behavior: `next_short_name` picks the highest existing N + 1, so files are preserved unless they happen to collide on N. Skipped for v1.
- Should clicking a queue row re-seek to that clip's IN marker and load its (in, out) into `self.in_marker` / `self.out_marker`? Lets the user revisit a queued clip. Easy to add later; v1 is display-only.
