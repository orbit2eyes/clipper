## Why

Content creators need to extract 9:16 portrait segments from landscape source videos for YouTube Shorts. The current path is either ffmpeg CLI (steep, error-prone) or heavyweight editors like Premiere / DaVinci (oversized, slow for a 60-second slice). Neither fits "I have a one-hour interview, I want three Shorts out of it" — that work is small, frequent, and should feel instant.

## What Changes

- Add a desktop GUI application named `clipper` for Linux and Windows.
- User loads a video file (any source aspect, typically 16:9 landscape).
- User drags a 9:16 crop window over the source preview to pick the framing that survives into the Short.
- User scrubs IN / OUT markers on a single timeline to select the time window.
- App re-encodes the selected region with the user crop to 1080x1920 H.264 + AAC, writes `short_NNN.mp4` to a chosen output directory, auto-incrementing the counter.
- Ship ffmpeg as a bundled per-platform binary; no system ffmpeg dependency required at runtime.
- Single Rust binary under 30 MB. egui-based GUI, no Chromium, no Electron, no webview.

Out of scope for v1: multi-clip timeline, effects, text overlays, transitions, color grading, AI subject-tracking crop, batch UI, real-time preview during crop drag, project files, undo history.

## Capabilities

### New Capabilities

- `clip-load`: open a video file from disk, parse its metadata (resolution, frame rate, duration, codec, audio tracks) for the rest of the app to consume.
- `clip-preview`: decode and display source frames in a preview area; render the user's draggable 9:16 crop window as an overlay; let the user reposition and resize the crop window with the mouse.
- `clip-trim`: scrub a single timeline track, set IN and OUT markers, snap to nearest frame, display current time / total duration.
- `clip-export`: take the source path, IN/OUT markers, and crop window; invoke ffmpeg with the right filter graph; produce a 1080x1920 H.264 + AAC MP4; write it as `short_NNN.mp4` in the chosen output directory, auto-incrementing NNN.

### Modified Capabilities

None. This is a greenfield project; no existing specs are touched.

## Impact

- New crate dependencies in `Cargo.toml`: `egui`, `eframe`, `ffmpeg-sidecar` or direct child-process invocation, `serde` for project state, `image` for thumbnail generation.
- New bundled binary: ffmpeg for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`, downloaded at build time.
- New top-level project layout: `src/`, `assets/`, `dist/`.
- Codec contract: H.264 baseline profile, yuv420p, 1080x1920, CRF 23 default, AAC-LC audio at 128 kbps, MP4 container with `+faststart` flag.
- No network calls. No telemetry. No external API. No login.
- Build targets: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`. Cross-compilation path TBD.
