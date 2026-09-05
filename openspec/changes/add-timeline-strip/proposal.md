## Why

clipper currently shows the source duration only as a numeric timecode in the transport bar. To find IN/OUT markers the user has to click on the preview (which jumps the playhead but doesn't move the markers) and then click Set IN / Set OUT. There's no spatial sense of "where in the video am I" or "where do the markers sit relative to the total duration." This adds a visual timeline strip below the preview that shows the full source duration as a horizontal bar with the playhead, IN/OUT markers, and the highlighted cut region — letting the user see and manipulate the cut at a glance.

## What Changes

- Add a horizontal timeline strip below the preview, in the central panel, above the existing transport bar.
- The strip shows: source duration as the full bar width, IN and OUT markers as draggable yellow triangles at the top edge, the playhead as a white vertical line, and the cut region (IN to OUT) as a tinted band.
- Click anywhere on the bar to seek the playhead to that point; drag the bar for continuous seek. Frame-snap to the source's frame rate when known.
- Drag the IN triangle to move the IN marker; drag the OUT triangle to move the OUT marker. Both frame-snapped and clamped to `[0, OUT]` and `[IN, duration]` respectively.
- Any seek or marker drag stops playback if it was running (predictable: bar manipulation doesn't race with the play thread).
- Edge cases: empty bar when `duration_secs <= 0`; no cut band when IN == OUT; no frame snap when source fps is 0 or negative.

## Capabilities

### New Capabilities

None. The visual strip is a UI-only addition within existing capabilities.

### Modified Capabilities

- `clip-trim`: add requirements describing the timeline strip's visual elements (cut band, playhead line, IN/OUT triangles) and interactions (click-to-seek, drag triangles). Existing requirements about Set IN/Set OUT buttons, IN/OUT display, and timeline behavior are preserved.

## Impact

- One new function in `src/main.rs`: `timeline_strip(&mut self, ui: &mut egui::Ui, avail: egui::Vec2)`.
- One new call site in the central panel, between the existing preview rendering and the bottom transport panel.
- One new delta spec at `openspec/changes/add-timeline-strip/specs/clip-trim/spec.md` with `## MODIFIED Requirements` adding the timeline-strip scenarios to existing `clip-trim` requirements.
- No new dependencies. No API changes. No effect on export pipeline.
- Affects ~150 lines of code in `src/main.rs`; everything else unchanged.
