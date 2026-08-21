use crate::model::{AssetKind, Clip, ClipTransform, FitMode, Project};
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

fn preview_dimensions(project: &Project) -> (u32, u32) {
    let width = project.width.max(2) as f64;
    let height = project.height.max(2) as f64;
    let scale = (960.0 / width).min(960.0 / height).min(1.0);
    let even = |value: f64| ((value * scale).round() as u32).max(2) / 2 * 2;
    (even(width), even(height))
}

fn transform_filter(width: u32, height: u32, transform: &ClipTransform) -> String {
    let mut filters = Vec::new();
    match transform.rotation_degrees {
        90 => filters.push("transpose=clock".to_string()),
        180 => filters.extend(["hflip".to_string(), "vflip".to_string()]),
        270 => filters.push("transpose=cclock".to_string()),
        _ => {}
    }
    match transform.fit {
        FitMode::Contain => filters.push(format!(
            "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black"
        )),
        FitMode::Cover => filters.push(format!(
            "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}:(iw-ow)*{:.6}:(ih-oh)*{:.6}",
            transform.position_x, transform.position_y
        )),
    }
    filters.push("setsar=1".to_string());
    filters.join(",")
}

fn extract_frame(media: &Path, seconds: f64, filter: &str, output: &Path) -> Result<Output> {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{seconds:.6}"))
        .arg("-i")
        .arg(media)
        .args(["-frames:v", "1", "-vf", filter])
        .arg(output)
        .output()
        .context("ffmpeg is required; install it with `brew install ffmpeg`")
}

fn contain_thumbnail(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let scale = (width as f64 / source.width().max(1) as f64)
        .min(height as f64 / source.height().max(1) as f64);
    let resized_width = ((source.width() as f64 * scale).round() as u32).max(1);
    let resized_height = ((source.height() as f64 * scale).round() as u32).max(1);
    let resized = imageops::resize(
        source,
        resized_width,
        resized_height,
        imageops::FilterType::Triangle,
    );
    let mut canvas = RgbaImage::from_pixel(width, height, Rgba([10, 12, 16, 255]));
    imageops::overlay(
        &mut canvas,
        &resized,
        ((width - resized_width) / 2) as i64,
        ((height - resized_height) / 2) as i64,
    );
    canvas
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
    let (preview_width, preview_height) = preview_dimensions(project);
    let Some(clip) = selected_video_clip(project, frame) else {
        let image = ImageBuffer::from_pixel(preview_width, preview_height, Rgba([15, 18, 24, 255]));
        DynamicImage::ImageRgba8(image).save(&output)?;
        return Ok(output);
    };
    let asset = project
        .asset(&clip.asset_id)
        .context("clip asset missing")?;
    let source_frame = clip.source_in_frame + frame - clip.start_frame;
    let source_seconds = project.fps.frames_to_seconds(source_frame);
    let media = resolve_asset_path(root, asset);
    let filter = transform_filter(preview_width, preview_height, &clip.transform);
    let mut result = extract_frame(&media, source_seconds, &filter, &output)?;
    if result.status.success() && !output.is_file() && source_seconds > 0.0 {
        // Variable/source frame rates can put the nominal final project frame
        // just beyond the source's last timestamp. Retry one project frame
        // earlier instead of turning an otherwise valid filmstrip into a 500.
        let previous = (source_seconds - 1.0 / project.fps.as_f64()).max(0.0);
        result = extract_frame(&media, previous, &filter, &output)?;
    }
    ensure_success("ffmpeg frame extraction", &result)?;
    if !output.is_file() {
        bail!("ffmpeg frame extraction produced no frame");
    }
    Ok(output)
}

pub fn thumbnail(root: &Path, asset: &crate::model::Asset) -> Result<PathBuf> {
    let output = root.join(format!(".te/cache/thumbnails/{}.jpg", asset.id));
    if output.is_file() {
        return Ok(output);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    if asset.kind == AssetKind::Audio {
        let image = ImageBuffer::from_pixel(480, 270, Rgba([24, 28, 36, 255]));
        DynamicImage::ImageRgba8(image).save(&output)?;
        return Ok(output);
    }
    let result = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "0.1",
            "-i",
        ])
        .arg(resolve_asset_path(root, asset))
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=480:270:force_original_aspect_ratio=increase,crop=480:270",
        ])
        .arg(&output)
        .output()?;
    ensure_success("ffmpeg thumbnail extraction", &result)?;
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
        let thumb = contain_thumbnail(&image::open(path)?.to_rgba8(), 240, 135);
        imageops::overlay(&mut strip, &thumb, (index as i64) * 240, 0);
    }
    DynamicImage::ImageRgba8(strip).save(&output)?;
    Ok(output)
}

pub fn clip_filmstrip(
    root: &Path,
    project: &Project,
    clip: &Clip,
    count: usize,
) -> Result<PathBuf> {
    let count = count.clamp(2, 20);
    let output = root.join(format!(
        ".te/cache/clip-filmstrips/r{:010}-{}-{count}.jpg",
        project.revision, clip.id
    ));
    if output.is_file() {
        return Ok(output);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut strip = RgbaImage::from_pixel((count as u32) * 160, 90, Rgba([10, 12, 16, 255]));
    for index in 0..count {
        let position = index as f64 / (count - 1) as f64;
        let offset = ((clip.duration_frames.saturating_sub(1)) as f64 * position).round() as i64;
        let path = frame_at(root, project, clip.start_frame + offset, None)?;
        let thumb = contain_thumbnail(&image::open(path)?.to_rgba8(), 160, 90);
        imageops::overlay(&mut strip, &thumb, (index as i64) * 160, 0);
    }
    DynamicImage::ImageRgba8(strip).save(&output)?;
    Ok(output)
}

pub fn clip_waveform(root: &Path, project: &Project, clip: &Clip) -> Result<PathBuf> {
    let output = root.join(format!(
        ".te/cache/waveforms/r{:010}-{}.png",
        project.revision, clip.id
    ));
    if output.is_file() {
        return Ok(output);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let asset = project
        .asset(&clip.asset_id)
        .context("clip asset missing")?;
    if !asset.has_audio {
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1200, 100, Rgba([0, 0, 0, 0])))
            .save(&output)?;
        return Ok(output);
    }
    let source_start = project.fps.frames_to_seconds(clip.source_in_frame);
    let duration = project
        .fps
        .frames_to_seconds(clip.duration_frames)
        .max(0.04);
    let result = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{source_start:.6}"))
        .arg("-t")
        .arg(format!("{duration:.6}"))
        .arg("-i")
        .arg(resolve_asset_path(root, asset))
        .args([
            "-filter_complex",
            "aformat=channel_layouts=mono,showwavespic=s=1200x100:colors=0xD8DEFF,format=rgba,colorkey=black:0.02:0.0",
            "-frames:v",
            "1",
        ])
        .arg(&output)
        .output()?;
    ensure_success("ffmpeg waveform generation", &result)?;
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
            let visual = transform_filter(width, height, &clip.transform);
            filters.push(format!(
                "[{input}:v:0]trim=start={source_start:.6}:end={source_end:.6},setpts=PTS-STARTPTS+{timeline_start:.6}/TB,{visual}[v{input}]"
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
