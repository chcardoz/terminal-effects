use crate::{edit, media, project};
use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const INDEX_HTML: &str = include_str!("../web/index.html");
const STYLES_CSS: &str = include_str!("../web/styles.css");
const APP_JS: &str = include_str!("../web/app.js");

pub struct EditorServer {
    pub url: String,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<()>>>,
}

impl EditorServer {
    pub fn start(project_path: &Path, port: u16) -> Result<Self> {
        let listener = Server::http(("127.0.0.1", port)).map_err(|error| anyhow!(error))?;
        let address = listener
            .server_addr()
            .to_ip()
            .context("editor server did not bind an IP address")?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let prefix = format!("/session/{token}/");
        let url = format!("http://127.0.0.1:{}{prefix}", address.port());
        let project_path = project_path.to_path_buf();
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("terminal-effects-http".into())
            .spawn(move || serve_loop(listener, &project_path, &prefix, &thread_shutdown))?;
        Ok(Self {
            url,
            shutdown,
            thread: Some(thread),
        })
    }

    pub fn wait(mut self) -> Result<()> {
        let thread = self
            .thread
            .take()
            .context("editor server already stopped")?;
        thread
            .join()
            .map_err(|_| anyhow!("editor server thread panicked"))?
    }
}

impl Drop for EditorServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_loop(
    server: Server,
    project_path: &Path,
    prefix: &str,
    shutdown: &AtomicBool,
) -> Result<()> {
    while !shutdown.load(Ordering::Acquire) {
        let Some(request) = server.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        if let Err(error) = route(request, project_path, prefix) {
            eprintln!("te: editor request failed: {error:#}");
        }
    }
    Ok(())
}

fn route(mut request: Request, project_path: &Path, prefix: &str) -> Result<()> {
    let requested = request.url().to_string();
    let Some(relative) = requested.strip_prefix(prefix) else {
        return respond_text(
            request,
            StatusCode(404),
            "text/plain; charset=utf-8",
            "Not found",
        );
    };
    let (path, query) = relative.split_once('?').unwrap_or((relative, ""));
    let result = match (request.method(), path) {
        (&Method::Get, "") | (&Method::Get, "index.html") => respond_text(
            request,
            StatusCode(200),
            "text/html; charset=utf-8",
            INDEX_HTML,
        ),
        (&Method::Get, "styles.css") => respond_text(
            request,
            StatusCode(200),
            "text/css; charset=utf-8",
            STYLES_CSS,
        ),
        (&Method::Get, "app.js") => respond_text(
            request,
            StatusCode(200),
            "text/javascript; charset=utf-8",
            APP_JS,
        ),
        (&Method::Get, icon) if icon.starts_with("icons/") => serve_icon(request, icon),
        (&Method::Get, "api/project") => project_response(request, project_path),
        (&Method::Get, "frame") => frame_response(request, project_path, query),
        (&Method::Get, media_path) if media_path.starts_with("media/") => {
            asset_file_response(request, project_path, &media_path[6..], false)
        }
        (&Method::Get, thumb_path) if thumb_path.starts_with("thumbnail/") => {
            asset_file_response(request, project_path, &thumb_path[10..], true)
        }
        (&Method::Post, "api/playhead") => {
            let result = (|| {
                let body: FrameBody = read_json(&mut request)?;
                let mut value = project::load(project_path)?;
                value.playhead_frame = body.frame.clamp(0, value.duration_frames());
                project::save(project_path, &value)?;
                Ok(json!({ "ok": true }))
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/select") => {
            let result = (|| {
                let body: ClipBody = read_json(&mut request)?;
                let mut value = project::load(project_path)?;
                let index = value.resolve_clip_index(&body.clip_id)?;
                value.selected_clip_id = Some(value.clips[index].id.clone());
                project::save(project_path, &value)?;
                Ok(json!({ "ok": true }))
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/split") => {
            let result = (|| {
                let body: ClipFrameBody = read_json(&mut request)?;
                edit::split(project_path, &body.clip_id, body.frame)
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/move") => {
            let result = (|| {
                let body: MoveBody = read_json(&mut request)?;
                edit::move_clip(
                    project_path,
                    &body.clip_id,
                    body.track_id.as_deref(),
                    body.frame,
                )
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/trim") => {
            let result = (|| {
                let body: TrimBody = read_json(&mut request)?;
                edit::trim(
                    project_path,
                    &body.clip_id,
                    body.start_frame,
                    body.duration_frames,
                    body.source_in_frame,
                )
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/remove") => {
            let result = (|| {
                let body: ClipBody = read_json(&mut request)?;
                edit::remove(project_path, &body.clip_id)
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/undo") => respond_api_result(request, edit::undo(project_path)),
        (&Method::Post, "api/redo") => respond_api_result(request, edit::redo(project_path)),
        (&Method::Post, "api/import") => {
            let result = (|| {
                let body: ImportBody = read_json(&mut request)?;
                if body.paths.is_empty() {
                    bail!("enter at least one media path");
                }
                let mut value = project::load(project_path)?;
                let paths = body
                    .paths
                    .into_iter()
                    .map(PathBuf::from)
                    .collect::<Vec<_>>();
                let ids = project::import_paths(project_path, &mut value, &paths)?;
                Ok(json!({ "ok": true, "importedAssetIds": ids }))
            })();
            respond_api_result(request, result)
        }
        (&Method::Post, "api/export") => {
            let result = (|| {
                let value = project::load(project_path)?;
                let root = project_path
                    .parent()
                    .context("project file has no parent")?;
                let output = root.join(".te/exports/export.mp4");
                media::export(root, &value, &output)?;
                Ok(json!({ "ok": true, "path": output }))
            })();
            respond_api_result(request, result)
        }
        _ => respond_json(request, StatusCode(404), &json!({ "error": "Not found" })),
    };
    if let Err(error) = result {
        // Request validation and edit errors belong in the UI, not in a dropped connection.
        // At this point request ownership may already be consumed by a response, so only
        // propagate I/O response errors; handler errors are caught before responding below.
        return Err(error);
    }
    Ok(())
}

fn project_response(request: Request, project_path: &Path) -> Result<()> {
    let value = project::load(project_path)?;
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let mut serialized = serde_json::to_value(&value)?;
    let assets = serialized["assets"]
        .as_array_mut()
        .context("serialized assets are not an array")?;
    for asset in assets {
        let id = asset["id"].as_str().unwrap_or_default().to_string();
        asset["mediaUrl"] = json!(format!("media/{id}"));
        asset["thumbnailUrl"] = json!(format!("thumbnail/{id}"));
    }
    respond_json(
        request,
        StatusCode(200),
        &json!({
            "project": serialized,
            "root": root,
            "projectFile": project_path,
            "durationFrames": value.duration_frames(),
            "fpsValue": value.fps.as_f64(),
            "renderer": "chromium-offscreen"
        }),
    )
}

fn frame_response(request: Request, project_path: &Path, query: &str) -> Result<()> {
    let value = project::load(project_path)?;
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let frame = query
        .split('&')
        .find_map(|part| part.strip_prefix("frame="))
        .and_then(|frame| frame.parse::<i64>().ok())
        .unwrap_or(value.playhead_frame)
        .clamp(0, value.duration_frames());
    let output = media::frame_at(root, &value, frame, None)?;
    respond_file(request, &output, "image/png")
}

fn asset_file_response(
    request: Request,
    project_path: &Path,
    asset_id: &str,
    thumbnail: bool,
) -> Result<()> {
    let value = project::load(project_path)?;
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let asset = value
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .with_context(|| format!("asset not found: {asset_id}"))?;
    if thumbnail {
        let output = media::thumbnail(root, asset)?;
        respond_file(request, &output, "image/jpeg")
    } else {
        let path = project::resolve_asset_path(root, asset);
        respond_file(request, &path, content_type(&path))
    }
}

fn respond_file(request: Request, path: &Path, content_type: &str) -> Result<()> {
    let mut file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let total = file.metadata()?.len();
    let range = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Range"))
        .and_then(|header| parse_range(header.value.as_str(), total));
    let (start, end, status) = range
        .map(|(start, end)| (start, end, StatusCode(206)))
        .unwrap_or((0, total.saturating_sub(1), StatusCode(200)));
    let length = if total == 0 { 0 } else { end - start + 1 };
    file.seek(SeekFrom::Start(start))?;
    let mut headers = vec![
        header("Content-Type", content_type)?,
        header("Accept-Ranges", "bytes")?,
        header("Cache-Control", "private, max-age=3600")?,
    ];
    if range.is_some() {
        headers.push(header(
            "Content-Range",
            &format!("bytes {start}-{end}/{total}"),
        )?);
    }
    let response = Response::new(
        status,
        headers,
        file.take(length),
        Some(length as usize),
        None,
    );
    request.respond(response)?;
    Ok(())
}

fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let value = value.strip_prefix("bytes=")?.split(',').next()?;
    let (start, end) = value.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().ok()?.min(total - 1)
    };
    (start <= end && start < total).then_some((start, end))
}

fn serve_icon(request: Request, icon: &str) -> Result<()> {
    let Some(bytes) = icon_bytes(icon) else {
        return respond_text(request, StatusCode(404), "text/plain", "Not found");
    };
    let response = Response::from_data(bytes)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "image/svg+xml")?)
        .with_header(header("Cache-Control", "public, max-age=86400")?);
    request.respond(response)?;
    Ok(())
}

macro_rules! icons {
    ($name:expr, $( $file:literal ),+ $(,)?) => {
        match $name {
            $(concat!("icons/", $file, ".svg") => Some(include_bytes!(concat!("../assets/icons/", $file, ".svg")).as_slice()),)+
            _ => None,
        }
    };
}

fn icon_bytes(name: &str) -> Option<&'static [u8]> {
    icons!(
        name,
        "film",
        "folder",
        "pause",
        "upload",
        "volume-x",
        "music-2",
        "mouse-pointer-2",
        "eye",
        "skip-back",
        "play",
        "step-forward",
        "skip-forward",
        "download",
        "magnet",
        "sliders-horizontal",
        "lock",
        "step-back",
        "chevron-down",
        "scissors",
        "volume-2",
        "search",
        "undo-2",
        "redo-2",
        "trash-2",
    )
}

fn respond_text(
    request: Request,
    status: StatusCode,
    content_type: &str,
    body: &str,
) -> Result<()> {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type)?)
        .with_header(header("Cache-Control", "no-store")?);
    request.respond(response)?;
    Ok(())
}

fn respond_json<T: serde::Serialize>(
    request: Request,
    status: StatusCode,
    value: &T,
) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let response = Response::from_data(body)
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json; charset=utf-8")?)
        .with_header(header("Cache-Control", "no-store")?);
    request.respond(response)?;
    Ok(())
}

fn respond_api_result<T: serde::Serialize>(request: Request, result: Result<T>) -> Result<()> {
    match result {
        Ok(value) => respond_json(request, StatusCode(200), &value),
        Err(error) => respond_json(
            request,
            StatusCode(400),
            &json!({ "error": format!("{error:#}") }),
        ),
    }
}

fn header(name: &str, value: &str) -> Result<Header> {
    Header::from_bytes(name, value).map_err(|_| anyhow!("invalid HTTP header {name}"))
}

fn read_json<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T> {
    let mut body = String::new();
    request
        .as_reader()
        .take(1_048_576)
        .read_to_string(&mut body)?;
    serde_json::from_str(&body).context("invalid JSON request")
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("mp3") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameBody {
    frame: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipBody {
    clip_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipFrameBody {
    clip_id: String,
    frame: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveBody {
    clip_id: String,
    track_id: Option<String>,
    frame: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrimBody {
    clip_id: String,
    start_frame: i64,
    source_in_frame: i64,
    duration_frames: i64,
}

#[derive(Deserialize)]
struct ImportBody {
    paths: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_and_bounded_ranges() {
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=120-", 100), None);
    }
}
