use crate::model::{AssetKind, Project};
use crate::project::resolve_asset_path;
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage, imageops};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Clone)]
pub struct MediaProbe {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub has_video: bool,
    pub has_audio: bool,
}

#[derive(Deserialize)]
struct ProbeOutput {
    streams: Vec<ProbeStream>,
    format: ProbeFormat,
}

#[derive(Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
}

#[derive(Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}

pub fn is_media_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4"
                    | "mov"
                    | "mkv"
                    | "webm"
                    | "m4v"
                    | "avi"
                    | "wav"
                    | "mp3"
                    | "m4a"
                    | "aac"
                    | "flac"
                    | "ogg"
            )
        })
}

pub fn probe_media(path: &Path) -> Result<MediaProbe> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .with_context(|| "ffprobe is required; install it with `brew install ffmpeg`")?;
    ensure_success("ffprobe", &output)?;
    let parsed: ProbeOutput = serde_json::from_slice(&output.stdout)?;
    let video = parsed
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("video"));
    let has_audio = parsed
        .streams
        .iter()
        .any(|stream| stream.codec_type.as_deref() == Some("audio"));
    let duration = parsed
        .format
        .duration
        .as_deref()
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| {
            parsed
                .streams
                .iter()
                .filter_map(|stream| stream.duration.as_deref()?.parse::<f64>().ok())
                .reduce(f64::max)
        })
        .context("media duration is unavailable")?;
    Ok(MediaProbe {
        duration,
        width: video.and_then(|stream| stream.width).unwrap_or(0),
        height: video.and_then(|stream| stream.height).unwrap_or(0),
        has_video: video.is_some(),
        has_audio,
    })
}

fn ensure_success(tool: &str, output: &Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail = stderr
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    bail!("{tool} failed:\n{tail}")
}

fn selected_video_clip(project: &Project, frame: i64) -> Option<&crate::model::Clip> {
    project
        .clips
        .iter()
        .filter(|clip| clip.start_frame <= frame && frame < clip.end_frame())
        .rfind(|clip| {
            project
                .asset(&clip.asset_id)
                .is_some_and(|asset| asset.kind == AssetKind::Video)
        })
}

pub fn frame_at(
    root: &Path,
    project: &Project,
    frame: i64,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let output = output.map(Path::to_path_buf).unwrap_or_else(|| {
        root.join(format!(
            ".te/cache/frames/r{:010}-frame-{frame:010}.png",
            project.revision
        ))
    });
    if output.is_file() {
        return Ok(output);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let Some(clip) = selected_video_clip(project, frame) else {
        let image = ImageBuffer::from_pixel(640, 360, Rgba([15, 18, 24, 255]));
        DynamicImage::ImageRgba8(image).save(&output)?;
        return Ok(output);
    };
    let asset = project
        .asset(&clip.asset_id)
        .context("clip asset missing")?;
    let source_frame = clip.source_in_frame + frame - clip.start_frame;
    let source_seconds = project.fps.frames_to_seconds(source_frame);
    let media = resolve_asset_path(root, asset);
    let result = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{source_seconds:.6}"))
        .arg("-i")
        .arg(media)
        .args(["-frames:v", "1", "-vf", "scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2:color=black"])
        .arg(&output)
        .output()?;
    ensure_success("ffmpeg frame extraction", &result)?;
    Ok(output)
}

pub fn filmstrip(
    root: &Path,
    project: &Project,
    start: i64,
    end: i64,
    count: usize,
    output: Option<&Path>,
) -> Result<PathBuf> {
    if end <= start {
        bail!("filmstrip end must be after start");
    }
    let count = count.clamp(2, 20);
    let output = output.map(Path::to_path_buf).unwrap_or_else(|| {
        root.join(format!(
            ".te/cache/filmstrips/r{:010}-{start:010}-{end:010}-{count}.jpg",
            project.revision
        ))
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut strip = RgbaImage::from_pixel((count as u32) * 240, 135, Rgba([10, 12, 16, 255]));
    for index in 0..count {
        let position = index as f64 / (count - 1) as f64;
        let frame = start + ((end - start) as f64 * position).round() as i64;
        let path = frame_at(root, project, frame, None)?;
        let thumb = imageops::resize(
            &image::open(path)?.to_rgba8(),
            240,
            135,
            imageops::FilterType::Triangle,
        );
        imageops::overlay(&mut strip, &thumb, (index as i64) * 240, 0);
    }
    DynamicImage::ImageRgba8(strip).save(&output)?;
    Ok(output)
}

pub fn export(root: &Path, project: &Project, output: &Path) -> Result<()> {
    if project.clips.is_empty() {
        bail!("timeline is empty");
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let duration = project
        .fps
        .frames_to_seconds(project.duration_frames())
        .max(0.04);
    let fps = project.fps.as_f64();
    let width = project.width.max(2) / 2 * 2;
    let height = project.height.max(2) / 2 * 2;
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-y"]);
    for clip in &project.clips {
        let asset = project
            .asset(&clip.asset_id)
            .context("clip asset missing")?;
        command.arg("-i").arg(resolve_asset_path(root, asset));
    }

    let mut filters = vec![format!(
        "color=c=0x101318:s={width}x{height}:r={fps:.6}:d={duration:.6}[base0]"
    )];
    let mut video_layer = 0usize;
    let mut audio_labels = Vec::new();
    for (input, clip) in project.clips.iter().enumerate() {
        let asset = project
            .asset(&clip.asset_id)
            .context("clip asset missing")?;
        let source_start = project.fps.frames_to_seconds(clip.source_in_frame);
        let source_end = project
            .fps
            .frames_to_seconds(clip.source_in_frame + clip.duration_frames);
        let timeline_start = project.fps.frames_to_seconds(clip.start_frame);
        let timeline_end = project.fps.frames_to_seconds(clip.end_frame());
        if asset.kind == AssetKind::Video {
            filters.push(format!(
                "[{input}:v:0]trim=start={source_start:.6}:end={source_end:.6},setpts=PTS-STARTPTS+{timeline_start:.6}/TB,scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black[v{input}]"
            ));
            filters.push(format!(
                "[base{video_layer}][v{input}]overlay=eof_action=pass:shortest=0:enable='between(t,{timeline_start:.6},{timeline_end:.6})'[base{}]",
                video_layer + 1
            ));
            video_layer += 1;
        }
        if asset.has_audio {
            let delay_ms = (timeline_start * 1000.0).round() as i64;
            filters.push(format!(
                "[{input}:a:0]atrim=start={source_start:.6}:end={source_end:.6},asetpts=PTS-STARTPTS,adelay=delays={delay_ms}:all=1[a{input}]"
            ));
            audio_labels.push(format!("[a{input}]"));
        }
    }
    filters.push(format!(
        "anullsrc=r=48000:cl=stereo:d={duration:.6}[asilence]"
    ));
    let audio_inputs = format!("[asilence]{}", audio_labels.join(""));
    filters.push(format!(
        "{audio_inputs}amix=inputs={}:duration=first:normalize=0[aout]",
        audio_labels.len() + 1
    ));
    command
        .arg("-filter_complex")
        .arg(filters.join(";"))
        .arg("-map")
        .arg(format!("[base{video_layer}]"))
        .args([
            "-map",
            "[aout]",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
        ])
        .arg(output);
    let result = command.output()?;
    ensure_success("ffmpeg export", &result)
}
