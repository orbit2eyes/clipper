## Context

The MVP ships with source load, crop drag, IN/OUT markers (set via buttons), playback, and export. The only way to see "where am I in the video" is the timecode text in the transport bar. The only way to set IN/OUT is clicking on the preview (sets playhead) and then clicking the Set IN/Set OUT button. There is no spatial representation of the source duration, the playhead position, or the marker placement.

This change adds a single horizontal strip between the preview and the transport bar that visualizes the full source duration as a scrubable bar with markers. No new state model — the strip is a pure projection of existing `playhead`, `in_marker`, `out_marker`, and `source.duration_secs`.

## Goals / Non-Goals

**Goals:**
- Visualize source duration, playhead, IN/OUT markers, and the cut region in one strip.
- Click on bar to seek the playhead; drag for continuous seek.
- Drag IN/OUT triangles to update markers directly.
- Frame-snap all seek/marker updates to the source's fps.
- Stop playback whenever the user manipulates the strip (no race with the play thread).
- Single Rust function `timeline_strip` in `src/main.rs`, called from the central panel.

**Non-Goals:**
- Waveform or thumbnail strip inside the timeline (real engineering lift; deferred).
- Drag-to-select (click-drag to set IN and OUT simultaneously).
- Keyboard shortcuts for IN/OUT at playhead (I, O, space, J/K/L).
- Zoomable / scrollable timeline (one fixed-width strip spanning the full duration).
- Animations on marker drag (immediate updates).
- Persisting bar state across sessions (strip is a pure projection of in-memory state).

## Decisions

### D1. One function, one call site

`timeline_strip(&mut self, ui, avail)` lives next to `poll_frame` and the preview rendering in `src/main.rs`. The central panel calls it after the preview block, before falling through to the bottom transport panel. No new module — small enough to live with the rest of the UI code.

### D2. Visual elements

| Element | Color | Position |
|---|---|---|
| Bar background | `Color32::from_gray(35)` | full rect |
| Cut region band | `Color32::from_rgb(60, 100, 160)` | rect from `in_x` to `out_x` |
| Playhead line | `Color32::WHITE`, 2px | vertical line at `play_x` |
| IN / OUT triangles | `Color32::YELLOW` | top edge, pointing down |
| Timecode labels | `Color32::from_gray(180)`, monospace 10pt | below bar |

Bar height: 36 px. Triangles: ±6 px wide, ~9 px tall.

### D3. Coordinate mapping

A closure `to_x(t: f64) -> f32` maps seconds to x-pixels: `bar_rect.left() + (t / duration as f32 * bar_rect.width())`. Used for cut band, playhead, and triangles. Inverse for drag updates: `(new_x - bar_rect.left()) / bar_rect.width() * duration`.

### D4. Frame-snap formula

`(t * fps).round() / fps` applied to every seek and marker update derived from the bar. Skipped if `fps <= 0`. The source's frame rate is already in `SourceMeta.fps` (parsed from `r_frame_rate`), so no extra probe work is needed.

### D5. Interaction handlers

- Bar background: `Sense::click_and_drag()`. On click or drag, compute `frac` from pointer x, snap to frame, set `playhead`, set `playing = false`, call `request_frame`.
- Each triangle: small hit rect (`±6 px`, ~12×12 px) with `Sense::drag()`. On drag, compute delta_x, convert to delta_t, snap to frame, clamp, update the marker. Stop playback on any update.
- Drag-past-other-marker: clamp to the other marker (no swap — predictable).

### D6. Playback integration

User manipulation of the strip always sets `self.playing = false`. Reasoning: the play thread (`play_stream`) is bound to the original IN/OUT range; changing IN/OUT or playhead mid-play would either need to restart the stream (more code) or desync visuals. Stop-on-manipulate is the predictable behavior and the user re-clicks Play to resume from the new state.

### D7. Existing Set IN/Set OUT buttons stay

Both the visual strip and the buttons reach the same markers. Keep the buttons — they remain useful for keyboard-free precise setting and for users who prefer clicking text labels.

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| egui Sense::drag on a tiny triangle is fiddly | Triangle hit rect is 12×12 px, generous for click target. Frame-snapping reduces mis-drag precision loss. |
| Stop-on-manipulate feels abrupt to power users | Documented in tooltip / spec; user re-clicks Play to resume. Smooth-drag-while-playing is a future change. |
| Frame-snap rounding on non-integer fps (29.97) drifts the visible playhead | Acceptable; the source's frame boundaries are what ffmpeg will encode from. |
| Bar background bleeds into surrounding panel | Use exact-sized allocation; no padding outside `bar_rect`. |
| Strip takes vertical space away from preview | 36 px is small relative to the 800 px window height. Transport bar remains the bottom row. |

## Migration Plan

N/A — additive UI change, no migration needed. Strip is a no-op when no source is loaded (`duration_secs == 0`).

## Open Questions

- Should the playhead triangle drag also stop playback? Current design: yes. Could revisit.
- Should the strip support click+drag to set both IN and OUT simultaneously (a "lasso" selection)? Out of scope per D7; could be added later.
- Tooltip on hover showing the exact time under the pointer? Trivial to add but deferred until asked for.
