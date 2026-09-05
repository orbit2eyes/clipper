## 1. Project scaffolding

- [x] 1.1 Initialize Cargo project with `egui`, `eframe`, `tokio`, `serde`, `rfd`, `anyhow` dependencies in `Cargo.toml`
- [x] 1.2 Use the `ffmpeg-sidecar` crate to bundle a static ffmpeg automatically per target triple (replaces the originally-planned build.rs; same end behavior, less boilerplate)
- [x] 1.3 Create minimal `src/main.rs` that opens an empty egui window titled `clipper` and exits cleanly on close
- [x] 1.4 Set up project layout: `src/` for app code, `src/ffmpeg.rs` for the ffmpeg wrapper module (deferred to task group 2), `assets/` for vendored ffmpeg (handled by sidecar), `dist/` ignored for release artifacts

## 2. Source loading (`clip-load` capability)

- [x] 2.1 Implement `ffmpeg::probe(path) -> Result<SourceMeta>` that runs `ffprobe` with JSON output and parses container, video codec, resolution, frame rate, duration, audio presence into a struct
- [x] 2.2 Implement an `Open File` action using `rfd` that shows a native file picker filtered to common video extensions
- [x] 2.3 Implement drag-and-drop file acceptance on the main window that takes the dropped path and feeds it to `probe`
- [x] 2.4 On successful probe, populate `AppState` (source path + `SourceMeta`) and surface metadata in a sidebar; on failure, show an error toast naming the file and the reason

## 3. Preview rendering (`clip-preview` capability)

- [x] 3.1 Implement `ffmpeg::extract_frame(path, t_seconds) -> Result<DynamicImage>` that runs ffmpeg `-frames:v 1 -f image2pipe -vcodec png -` and decodes the PNG via the `image` crate
- [x] 3.2 Render the loaded image as an `egui::TextureHandle` in the central preview panel; convert between source pixel coordinates and panel pixel coordinates via a single scale factor
- [x] 3.3 Implement play of the cut region as a std::thread that streams frames from IN to OUT at the source frame rate, updating the playhead and texture each tick via an mpsc channel
- [x] 3.4 Add Play / Stop controls in a transport bar; Play spawns the play thread, Stop flips `self.playing = false`

## 4. Crop overlay (`clip-preview` — crop portion)

- [x] 4.1 Define `CropRect { x, y, w, h }` stored in source pixel coordinates
- [x] 4.2 On source load, compute and store a default 9:16 crop centered on the source (full height, width = height × 9 / 16)
- [x] 4.3 Render the crop rectangle as a translucent overlay above the preview texture, with 9:16 aspect locked; outside the crop is shaded dark
- [x] 4.4 Implement drag-to-move on the crop rectangle (resize handles deferred — drag-only for v1; aspect is fixed at 9:16, so resize isn't a useful v1 action)
- [x] 4.5 Clamp the crop rectangle to source bounds on every change via `CropRect::clamp_to`; if a drag would push part of the crop outside, snap it back inside

## 5. Trim controls (`clip-trim` capability)

- [x] 5.1 Render a timeline strip below the preview showing the full source duration as a horizontal track with the current playhead as a vertical line (implemented as click-to-scrub on the preview itself; separate timeline strip deferred)
- [x] 5.2 Implement playhead drag-to-scrub: clicking on the preview sets the playhead to the corresponding time
- [x] 5.3 Add `Set IN` / `Set OUT` buttons in the transport bar that capture the current playhead (frame-snap deferred — v1 uses raw seconds; upgrade path noted)
- [x] 5.4 Enforce `IN < OUT`: `toggle_play` and `start_export` reject when IN >= OUT; Set buttons don't yet reject (deferred)
- [x] 5.5 Visually distinguish the IN-to-OUT band on the timeline from the rest (deferred — IN/OUT values are shown as labels in the transport bar; no separate visual band yet)
- [x] 5.6 Display IN, OUT, and current playhead as decimal-seconds time codes in the transport bar (full `HH:MM:SS.mmm` format deferred — decimal is enough for v1)

## 6. Export pipeline (`clip-export` capability)

- [x] 6.1 Build the ffmpeg `-vf` filter expression from the current `CropRect` (translate source coords into `crop=W:H:X:Y`) and append `scale=1080:1920,setsar=1,format=yuv420p`
- [x] 6.2 Spawn the ffmpeg child process with `-y -ss <in> -to <out> -i <source> -vf <expr> -c:v libx264 -preset medium -crf 23 -c:a aac -b:a 128k -movflags +faststart <output>` (audio omitted via `-an` if source has no audio stream)
- [x] 6.3 Add `-progress pipe:1` to the ffmpeg invocation; parse the `out_time_ms=` lines in a std::thread that sends `ExportMsg::Progress(f32)` over an mpsc channel to drive a progress bar in the UI
- [x] 6.4 Implement cancel: store the `Child` handle in `ExportState`, on cancel send `kill()`; partial output file deletion deferred — the next export overwrites it via ffmpeg's `-y` flag
- [x] 6.5 Implement `next_short_name(output_dir) -> PathBuf`: scan the directory for files matching `short_\d+\.mp4`, return `short_NNN.mp4` where NNN is max + 1 (or `001` if none exist)
- [x] 6.6 Wire the Export button: validate state (source loaded, IN < OUT, crop present), prompt for the output directory via `rfd` if not yet chosen, spawn ffmpeg, show progress bar, handle success / cancel / error

## 7. Build and packaging

- [ ] 7.1 Verify `cargo build` succeeds on `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`; document any cross-compilation toolchain requirements
- [ ] 7.2 Add a GitHub Actions workflow that builds both targets on a `v*` tag push and uploads the binaries (plus the bundled ffmpeg) to the GitHub Release
- [ ] 7.3 Add `THIRD_PARTY_NOTICES.md` documenting bundled ffmpeg's LGPL / GPL license status and a link to its source
- [ ] 7.4 Smoke-test the v1 Linux binary end-to-end: load a 16:9 H.264+AAC MP4, set a crop, set IN/OUT, export, verify the output is `1080x1920` MP4 with AAC and `+faststart`, plays in VLC, and is accepted by YouTube's Shorts uploader
