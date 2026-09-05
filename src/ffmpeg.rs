use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct SourceMeta {
    pub path: PathBuf,
    pub container: String,
    pub video_codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_secs: f64,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: FfprobeFormat,
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    #[serde(rename = "format_name")]
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    channels: Option<u32>,
}

pub fn probe(path: &Path) -> Result<SourceMeta> {
    let ffprobe = find_ffprobe();

    let output = Command::new(&ffprobe)
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to spawn ffprobe at {}", ffprobe.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe could not read {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
        .context("ffprobe returned malformed JSON")?;

    let video = parsed
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .ok_or_else(|| anyhow!("no video stream in {}", path.display()))?;

    let audio = parsed.streams.iter().find(|s| s.codec_type == "audio");

    let fps = video
        .r_frame_rate
        .as_deref()
        .and_then(parse_rational)
        .unwrap_or(0.0);

    let duration_secs = parsed
        .format
        .duration
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(SourceMeta {
        path: path.to_path_buf(),
        container: parsed.format.format_name.unwrap_or_default(),
        video_codec: video.codec_name.clone().unwrap_or_default(),
        width: video.width.unwrap_or(0),
        height: video.height.unwrap_or(0),
        fps,
        duration_secs,
        audio_codec: audio.and_then(|a| a.codec_name.clone()),
        audio_channels: audio.and_then(|a| a.channels),
    })
}

fn find_ffprobe() -> PathBuf {
    if let Ok(p) = std::env::var("CLIPPER_FFPROBE") {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    PathBuf::from(name)
}

fn parse_rational(s: &str) -> Option<f64> {
    let mut parts = s.split('/');
    let num: f64 = parts.next()?.parse().ok()?;
    let den: f64 = parts.next()?.parse().ok()?;
    if den == 0.0 { None } else { Some(num / den) }
}

fn find_ffmpeg() -> PathBuf {
    if let Ok(p) = std::env::var("CLIPPER_FFMPEG") {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    PathBuf::from(name)
}

/// Preview width for the on-screen preview pipeline. Smaller = faster decode,
/// less pipe bandwidth, faster texture upload. The export pipeline uses the
/// full source resolution.
pub const PREVIEW_WIDTH: u32 = 720;

fn preview_size(source_w: u32, source_h: u32) -> (u32, u32) {
    let w = PREVIEW_WIDTH;
    let h = ((source_h as f64 * w as f64 / source_w as f64).round() as u32) & !1;
    (w, h.max(2))
}

pub fn extract_frame(path: &Path, t_seconds: f64, source_w: u32, source_h: u32) -> Result<image::DynamicImage> {
    let ffmpeg = find_ffmpeg();
    let (pw, ph) = preview_size(source_w, source_h);
    let output = Command::new(&ffmpeg)
        .args([
            "-v", "error",
            "-ss", &format!("{:.3}", t_seconds),
            "-i", &path.to_string_lossy(),
            "-frames:v", "1",
            "-vf", &format!("scale={}:{}", pw, ph),
            "-f", "image2pipe",
            "-vcodec", "png",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to spawn ffmpeg at {}", ffmpeg.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg could not extract frame at {:.3}s from {}: {}",
            t_seconds,
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    image::load_from_memory(&output.stdout).context("could not decode extracted frame")
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub source: PathBuf,
    pub in_seconds: f64,
    pub out_seconds: f64,
    pub crop: (i32, i32, u32, u32), // x, y, w, h in source pixels
    pub output: PathBuf,
    pub has_audio: bool,
}

pub fn next_short_name(dir: &Path) -> PathBuf {
    let mut max_n = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Some(rest) = name.strip_prefix("short_").and_then(|s| s.strip_suffix(".mp4")) {
                    if let Ok(n) = rest.parse::<u32>() {
                        if n > max_n { max_n = n; }
                    }
                }
            }
        }
    }
    dir.join(format!("short_{:03}.mp4", max_n + 1))
}

pub fn spawn_export(req: ExportRequest) -> std::process::Child {
    let ffmpeg = find_ffmpeg();
    let (crop_x, crop_y, crop_w, crop_h) = req.crop;
    let vf = format!(
        "crop={}:{}:{}:{},scale=1080:1920,setsar=1,format=yuv420p",
        crop_w, crop_h, crop_x, crop_y
    );

    let mut cmd = Command::new(&ffmpeg);
    cmd.args([
        "-y",
        "-v", "error",
        "-progress", "pipe:1",
        "-ss", &format!("{:.3}", req.in_seconds),
        "-to", &format!("{:.3}", req.out_seconds),
        "-i", &req.source.to_string_lossy(),
        "-vf", &vf,
        "-c:v", "libx264",
        "-preset", "medium",
        "-crf", "23",
    ]);
    if req.has_audio {
        cmd.args(["-c:a", "aac", "-b:a", "128k"]);
    } else {
        cmd.arg("-an");
    }
    cmd.args(["-movflags", "+faststart"]);
    cmd.arg(&req.output);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.spawn().expect("failed to spawn ffmpeg")
}

/// Spawn a single ffmpeg process that streams raw RGBA frames from `in_seconds`
/// to `out_seconds` of `path` to stdout. Returns a channel that yields decoded
/// frames as `image::DynamicImage` values; the channel is bounded (4 frames)
/// so the producer blocks when the consumer is slow, naturally pacing playback.
///
/// The first stderr line (if any) is captured and forwarded through the
/// returned `Receiver<(image::DynamicImage, Option<String>)>` so the caller
/// can surface ffmpeg's own error message when the stream produces no frames.
pub fn play_stream(
    path: PathBuf,
    in_seconds: f64,
    out_seconds: f64,
    source_width: u32,
    source_height: u32,
    source_fps: f64,
) -> Result<std::sync::mpsc::Receiver<(image::DynamicImage, Option<String>)>> {
    let ffmpeg = find_ffmpeg();
    let (pw, ph) = preview_size(source_width, source_height);
    let mut cmd = Command::new(&ffmpeg);
    cmd.args([
        "-v", "error",
        "-ss", &format!("{:.3}", in_seconds),
        "-to", &format!("{:.3}", out_seconds),
        "-i", &path.to_string_lossy(),
        "-vf", &format!("scale={}:{}", pw, ph),
        "-f", "rawvideo",
        "-pix_fmt", "rgba",
        "-",
    ]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn {} for play stream", ffmpeg.display()))?;
    let stdout = child.stdout.take().context("ffmpeg stdout not piped")?;
    let stderr = child.stderr.take().context("ffmpeg stderr not piped")?;

    eprintln!(
        "[ffmpeg] play_stream: {} -ss {:.3} -to {:.3} -i {} -vf scale={}:{} -f rawvideo -pix_fmt rgba - (real-time pacing @ {:.2} fps)",
        ffmpeg.display(), in_seconds, out_seconds, path.display(), pw, ph, source_fps
    );

    let (tx, rx) = std::sync::mpsc::sync_channel::<(image::DynamicImage, Option<String>)>(4);

    std::thread::spawn(move || {
        // Drain stderr in a separate thread, capturing only the first line.
        let (etx, erx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                eprintln!("[ffmpeg stderr] {}", line);
                let _ = etx.send(line);
                break; // first line is enough
            }
        });

        use std::io::Read;
        use std::time::{Duration, Instant};
        let mut stdout = stdout;
        let bytes_per_frame = (pw as usize) * (ph as usize) * 4;
        let mut buf = vec![0u8; bytes_per_frame];

        // Real-time pacing: target wall-clock time for frame `i` is
        // `start + i / fps`. If we're ahead, sleep; if behind, send immediately.
        let frame_period = if source_fps > 0.0 {
            Duration::from_secs_f64(1.0 / source_fps)
        } else {
            Duration::from_millis(33)
        };
        let start = Instant::now();
        let mut i: u64 = 0;

        loop {
            match stdout.read_exact(&mut buf) {
                Ok(()) => {
                    if let Some(img) = image::RgbaImage::from_raw(pw, ph, buf.clone()) {
                        let msg = erx.try_recv().ok();
                        if tx.send((image::DynamicImage::ImageRgba8(img), msg)).is_err() {
                            break;
                        }
                        let target = start + frame_period.checked_mul(i as u32).unwrap_or(start.elapsed());
                        let now = Instant::now();
                        if now < target {
                            std::thread::sleep(target - now);
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = child.wait();
    });

    Ok(rx)
}
