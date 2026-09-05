## Context

Greenfield repository. Created today via `openspec init --tools pi`. No code, no specs, no source files yet. The user wants a single-binary desktop video editor named `clipper` that produces YouTube Shorts-ready clips from arbitrary-aspect source videos by letting the user pick a 9:16 crop window and trim IN / OUT markers. The product target is "I have a one-hour interview recording and I want three Shorts out of it without launching Premiere."

The repo's only existing artifacts are the OpenSpec scaffold (config + empty `specs/` and `changes/` directories) and the Pi skills directory.

## Goals / Non-Goals

**Goals:**
- Single Rust binary, runs on Linux x86_64 and Windows x86_64 with no system-level dependencies.
- GUI lets the user open a video, scrub a timeline, drag a 9:16 crop window over a preview, set IN / OUT, and export a `short_NNN.mp4`.
- Export produces a 1080x1920 H.264 + AAC MP4 ready for YouTube Shorts upload.
- Output filename auto-numbers within a chosen output directory.
- Build is reproducible; binary bundles ffmpeg so the user does not need ffmpeg installed system-wide.

**Non-Goals (v1):**
- Multi-clip timeline, multi-track audio, mixing.
- Visual effects, transitions, text overlays, lower-thirds, captions.
- Color grading, LUTs, HDR.
- AI subject-tracking smart crop.
- Batch UI (apply cuts to many files at once).
- Real-time smooth preview during crop drag.
- Project files, undo / redo history, autosave.
- macOS support.
- Network features: no telemetry, no login, no cloud rendering.

## Decisions

### D1. Language and GUI framework: Rust + egui / eframe

| | egui | Tauri | Electron | iced / Slint |
|---|---|---|---|---|
| Binary size | ~10 MB | ~20 MB + webview | ~150 MB+ | ~10 MB |
| Native look | OK (custom paint) | Good (webview) | Good (webview) | OK |
| Build matrix | simple (Cargo) | complex (rustc + Node + webview) | complex (Node + Chromium) | simple |
| Learning curve | immediate-mode is straightforward | web + Rust bridge | web | moderate |

Choice: **egui + eframe**. Pure Rust, smallest binary footprint, no webview runtime, single `cargo build` per platform, immediate-mode reduces state-plumbing code. Visual polish is acceptable for a utility tool; revisit only if feedback demands more.

### D2. Video engine: bundled ffmpeg child process

Alternatives considered:
- Direct libavcodec / libavformat Rust bindings — tighter coupling, smaller binary if ffmpeg symbols stripped, but a lot of unsafe code and a moving upstream API.
- Pure Rust (rav1e / libaom) — re-encoding only, not input demuxing, does not solve the input-format problem.
- GStreamer — heavyweight runtime, more dependencies than ffmpeg alone.

Choice: **ffmpeg as a bundled child process**. ffmpeg handles every input container and codec users will throw at it, has the exact filter graph we need (`crop`, `scale`, `format`, encode), and never needs to be reimplemented. ffmpeg binary is fetched at build time per target triple and shipped alongside the Rust binary; the app does not shell out to system ffmpeg, so the user never sees a "ffmpeg not found" error.

Trade-off: total install size is Rust binary (≈10 MB) + ffmpeg static build (≈30 MB) = ~40 MB per platform. Within the "<30 MB" aspirational goal we discussed in exploration, but only for the Rust half. Acceptable.

### D3. Preview rendering: ffmpeg-extracted frames on demand

Alternatives: link libavcodec directly for a real-time decoder, embed mpv, decode every frame on a timer.

Choice: **extract one frame at a time on scrub, decode-and-show on play of the selected cut region**. Source frame extraction goes through ffmpeg (`-ss <t> -frames:v 1 -i <src> -f image2pipe -`); the resulting PNG / raw RGB is blitted into an `egui::TextureHandle`. Crop overlay is rendered as egui primitives over the texture.

Trade-off: scrubbing shows stills, not motion. Acceptable because the workflow is "scrub to find IN / OUT, then play the cut region to verify" — playback of the cut region uses real-time frame streaming.

### D4. Crop semantics: pixel-precise rectangle + scale to 1080x1920

- Crop stored as `(x, y, w, h)` in source pixel coordinates.
- Aspect-locked to 9:16 by default; user can move and resize, the aspect is preserved automatically.
- Output always 1080x1920 (Shorts native).
- ffmpeg filter graph: `-vf "crop=W:H:X:Y,scale=1080:1920,setsar=1,format=yuv420p"`.

Trade-off: if source is not 16:9, the crop window's height may be clipped by the source bounds. UI must clamp the crop rectangle to the source frame.

### D5. Trim model: frame-accurate IN / OUT markers

- IN and OUT stored as `f64` seconds, snapped to the nearest frame at display time.
- Timeline UI: horizontal scrub bar, drag the playhead, click to set IN or OUT at the playhead.
- Export uses `-ss <in> -to <out>` placed BEFORE `-i` for fast keyframe-anchored seek (input may need `-noaccurate_seek` if precision matters; verify in prototype).

### D6. Output naming: `short_NNN.mp4` auto-numbered per output directory

- On export, scan the chosen output directory for existing `short_*.mp4` files, find the highest NNN, increment, write.
- Counter persistence is implicit (re-scan each export); no separate counter file in v1.

Trade-off: deleting a file mid-session shifts the next number. Acceptable; user-visible and predictable.

### D7. Export pipeline: single ffmpeg invocation

```
ffmpeg -y -ss <in> -to <out> -i <source> \
  -vf "crop=W:H:X:Y,scale=1080:1920,setsar=1,format=yuv420p" \
  -c:v libx264 -preset medium -crf 23 \
  -c:a aac -b:a 128k \
  -movflags +faststart \
  <output>
```

- One process from source to MP4. No intermediate files.
- Progress reported via `-progress pipe:1` (stdout `key=value` lines parsed by the Rust side to drive a progress bar).
- Cancellation: kill the ffmpeg child on user cancel; partial output file is removed.

### D8. State model: in-memory only

- All state lives in the egui app struct: loaded source path, source metadata, crop rectangle, IN, OUT, output directory, last-used settings.
- No project file. No persistence across app restarts.
- Trade-off: closing the app discards the in-progress cut. Acceptable for v1; revisit only if it bites.

## Architecture sketch

```
┌──────────────────────────────────────────────────────────────┐
│                        egui app                             │
│  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐  │
│  │  source    │  │  timeline  │  │  preview canvas        │  │
│  │  panel     │  │  + IN/OUT  │  │  + draggable 9:16 crop │  │
│  └────────────┘  └────────────┘  └────────────────────────┘  │
│                           │                                  │
│                  ┌────────▼────────┐                         │
│                  │   AppState      │  (in-memory)            │
│                  │   - source meta │                         │
│                  │   - crop rect   │                         │
│                  │   - in / out    │                         │
│                  │   - output dir  │                         │
│                  └────────┬────────┘                         │
│                           │                                  │
│         ┌─────────────────┼─────────────────┐                │
│         ▼                 ▼                 ▼                │
│  ┌────────────┐   ┌────────────┐   ┌────────────┐           │
│  │  ffmpeg    │   │  ffmpeg    │   │  ffmpeg    │           │
│  │  probe     │   │  extract   │   │  export    │           │
│  │  (once)    │   │  frames    │   │  (on save) │           │
│  └────────────┘   └────────────┘   └────────────┘           │
└──────────────────────────────────────────────────────────────┘
```

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| ffmpeg binary not found at runtime | Bundle statically, never shell out to system ffmpeg. Document bundled version in `--version` output. |
| Codec latency on long (>1h) sources | Use `-ss` before `-i` (input seek), not output seek. Show progress bar via ffmpeg `-progress pipe:1`. |
| Aspect ratio edge cases (square, vertical, ultra-wide source) | Clamp crop rect to source bounds in the GUI; ffmpeg filter expression will refuse out-of-bounds crops with a clear error. |
| Crop drag perf on 4K source | Render preview at 540x960 (downscale-on-extract); keep crop coordinates in source pixel space. |
| egui aesthetic vs polished UI | Acceptable for v1; revisit only if user feedback demands more. |
| No undo / no project file | Acceptable for v1; document as known limitation. |
| Counter collision across multiple clipper instances writing to the same dir | Document: only one clipper instance per output directory. Add file lock later if needed. |
| Bundled ffmpeg license (LGPL / GPL) | Document in LICENSE / README; provide source-code link for ffmpeg. Add to release notes. |

## Open Questions

- **Build pipeline**: GitHub Actions matrix vs local cross-compilation? Default: GH Actions (free for OSS, both targets in one workflow).
- **Progress parsing**: `-progress pipe:1` vs parsing ffmpeg's stderr `-stats` lines? Default: `-progress pipe:1`, easier to parse.
- **Aspect-locked vs free-form crop**: should the user be able to drop the 9:16 lock for non-Shorts use? Default: locked to 9:16 in v1, expose a toggle later.
- **Audio handling when source has no audio**: skip the audio stream silently or fail? Default: skip silently, log a notice.
- **First-frame behavior**: should the app auto-load the source and seek to 0, or show a drop zone until the user clicks Open? Default: drop zone, single-click Open.

## Migration Plan

N/A — greenfield. Ship v1 binaries via GitHub Releases for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`.
