mod edit;
mod editor;
mod media;
mod model;
mod project;
mod runtime;
mod server;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use model::{Project, format_timecode, parse_time};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "te",
    version,
    about = "Video editing, directly in the terminal"
)]
struct Cli {
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    #[arg(long, global = true, value_name = "PATH")]
    project: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<CommandKind>,
}

#[derive(Subcommand)]
enum CommandKind {
    Open {
        path: Option<PathBuf>,
    },
    Init {
        path: Option<PathBuf>,
    },
    /// Run the Chromium editor in a normal browser for development or remote access.
    Serve {
        path: Option<PathBuf>,
        #[arg(long, default_value_t = 4173)]
        port: u16,
    },
    /// Inspect or install the managed Chromium terminal runtime.
    Runtime {
        #[arg(long)]
        install: bool,
        #[arg(long)]
        json: bool,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
    Timeline {
        #[arg(long)]
        json: bool,
    },
    Clips {
        #[arg(long)]
        json: bool,
    },
    Import {
        files: Vec<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Frame {
        at: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Filmstrip {
        range: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long, default_value_t = 6)]
        count: usize,
        #[arg(long)]
        json: bool,
    },
    Screenshot {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Split {
        clip: String,
        at: String,
        #[arg(long)]
        json: bool,
    },
    Move {
        clip: String,
        #[arg(long)]
        track: Option<String>,
        #[arg(long)]
        at: String,
        #[arg(long)]
        json: bool,
    },
    Trim {
        clip: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        duration: String,
        #[arg(long = "source-in")]
        source_in: String,
        #[arg(long)]
        json: bool,
    },
    Remove {
        clip: String,
        #[arg(long)]
        json: bool,
    },
    Undo {
        #[arg(long)]
        json: bool,
    },
    Redo {
        #[arg(long)]
        json: bool,
    },
    Export {
        output: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusView<'a> {
    project: &'a str,
    root: String,
    revision: u64,
    editor: EditorView<'a>,
    timeline: TimelineSummary,
    assets: usize,
    clips: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorView<'a> {
    running: bool,
    playhead_frame: i64,
    playhead: String,
    selected_clip_id: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimelineSummary {
    fps: model::Fps,
    duration_frames: i64,
    duration: String,
    width: u32,
    height: u32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("te: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            let path = cli.path.unwrap_or_else(|| PathBuf::from("."));
            let project_path = project::open_or_create(&path)?;
            editor::run(&project_path)
        }
        Some(CommandKind::Open { path }) => {
            let project_path = project::open_or_create(path.as_deref().unwrap_or(Path::new(".")))?;
            editor::run(&project_path)
        }
        Some(CommandKind::Init { path }) => {
            let project_path = project::open_or_create(path.as_deref().unwrap_or(Path::new(".")))?;
            println!("{}", project_path.display());
            Ok(())
        }
        Some(CommandKind::Serve { path, port }) => {
            let project_path = project::open_or_create(path.as_deref().unwrap_or(Path::new(".")))?;
            editor::serve(&project_path, port)
        }
        Some(CommandKind::Runtime { install, json }) => {
            if install {
                let path = runtime::resolve()?;
                output(
                    json,
                    &serde_json::json!({ "ok": true, "path": path, "version": runtime::TERMINAL_BROWSER_VERSION }),
                )
            } else {
                output(json, &runtime::managed_status()?)
            }
        }
        Some(command) => run_command(command, cli.project),
    }
}

fn resolve_project(explicit: Option<PathBuf>) -> Result<PathBuf> {
    let start = explicit.unwrap_or(env::current_dir()?);
    project::find_project(&start)
}

fn run_command(command: CommandKind, explicit: Option<PathBuf>) -> Result<()> {
    let project_path = resolve_project(explicit)?;
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let mut project = project::load(&project_path)?;
    match command {
        CommandKind::Status { json } => {
            let view = status_view(root, &project);
            if json {
                project::print_json(&view)
            } else {
                print_status(&view);
                Ok(())
            }
        }
        CommandKind::Timeline { json } => {
            if json {
                project::print_json(&serde_json::json!({
                    "revision": project.revision,
                    "fps": project.fps,
                    "durationFrames": project.duration_frames(),
                    "tracks": project.tracks,
                    "clips": project.clips,
                }))
            } else {
                print_timeline(&project);
                Ok(())
            }
        }
        CommandKind::Clips { json } => {
            if json {
                project::print_json(
                    &serde_json::json!({ "revision": project.revision, "clips": project.clips }),
                )
            } else {
                for clip in &project.clips {
                    let name = project
                        .asset(&clip.asset_id)
                        .map(|asset| asset.name.as_str())
                        .unwrap_or("missing");
                    println!(
                        "{}  {}  {}  {}..{}  {}",
                        clip.id,
                        clip.track_id,
                        format_timecode(clip.start_frame, project.fps),
                        clip.start_frame,
                        clip.end_frame(),
                        name
                    );
                }
                Ok(())
            }
        }
        CommandKind::Import { files, json } => {
            if files.is_empty() {
                bail!("pass one or more media files");
            }
            let ids = project::import_paths(&project_path, &mut project, &files)?;
            output(
                json,
                &serde_json::json!({ "ok": true, "revision": project.revision, "importedAssetIds": ids }),
            )
        }
        CommandKind::Frame {
            at,
            output: path,
            json,
        } => {
            let frame = parse_time(&at, project.fps)?.clamp(0, project.duration_frames());
            let output_path = media::frame_at(root, &project, frame, path.as_deref())?;
            output(
                json,
                &serde_json::json!({ "ok": true, "frame": frame, "timecode": format_timecode(frame, project.fps), "path": output_path }),
            )
        }
        CommandKind::Filmstrip {
            range,
            output: path,
            count,
            json,
        } => {
            let (start, end) = range
                .split_once("..")
                .context("range must look like 10s..20s")?;
            let start = parse_time(start, project.fps)?;
            let end = parse_time(end, project.fps)?;
            let output_path = media::filmstrip(root, &project, start, end, count, path.as_deref())?;
            output(
                json,
                &serde_json::json!({ "ok": true, "startFrame": start, "endFrame": end, "count": count.clamp(2, 20), "path": output_path }),
            )
        }
        CommandKind::Screenshot { output: path, json } => {
            let output_path =
                media::frame_at(root, &project, project.playhead_frame, path.as_deref())?;
            output(
                json,
                &serde_json::json!({ "ok": true, "kind": "previewFrame", "revision": project.revision, "frame": project.playhead_frame, "path": output_path }),
            )
        }
        CommandKind::Split { clip, at, json } => {
            let frame = parse_time(&at, project.fps)?;
            output(json, &edit::split(&project_path, &clip, frame)?)
        }
        CommandKind::Move {
            clip,
            track,
            at,
            json,
        } => {
            let frame = parse_time(&at, project.fps)?;
            output(
                json,
                &edit::move_clip(&project_path, &clip, track.as_deref(), frame)?,
            )
        }
        CommandKind::Trim {
            clip,
            start,
            duration,
            source_in,
            json,
        } => {
            let start = parse_time(&start, project.fps)?;
            let duration = parse_time(&duration, project.fps)?;
            let source_in = parse_time(&source_in, project.fps)?;
            output(
                json,
                &edit::trim(&project_path, &clip, start, duration, source_in)?,
            )
        }
        CommandKind::Remove { clip, json } => output(json, &edit::remove(&project_path, &clip)?),
        CommandKind::Undo { json } => output(json, &edit::undo(&project_path)?),
        CommandKind::Redo { json } => output(json, &edit::redo(&project_path)?),
        CommandKind::Export { output: path, json } => {
            let path = if path.is_absolute() {
                path
            } else {
                env::current_dir()?.join(path)
            };
            media::export(root, &project, &path)?;
            output(
                json,
                &serde_json::json!({ "ok": true, "revision": project.revision, "path": path }),
            )
        }
        CommandKind::Open { .. }
        | CommandKind::Init { .. }
        | CommandKind::Serve { .. }
        | CommandKind::Runtime { .. } => unreachable!(),
    }
}

fn output<T: Serialize>(json: bool, value: &T) -> Result<()> {
    if json {
        project::print_json(value)
    } else {
        println!("{}", serde_json::to_string(value)?);
        Ok(())
    }
}

fn status_view<'a>(root: &Path, project: &'a Project) -> StatusView<'a> {
    StatusView {
        project: &project.name,
        root: root.display().to_string(),
        revision: project.revision,
        editor: EditorView {
            running: editor::session_running(root),
            playhead_frame: project.playhead_frame,
            playhead: format_timecode(project.playhead_frame, project.fps),
            selected_clip_id: project.selected_clip_id.as_deref(),
        },
        timeline: TimelineSummary {
            fps: project.fps,
            duration_frames: project.duration_frames(),
            duration: format_timecode(project.duration_frames(), project.fps),
            width: project.width,
            height: project.height,
        },
        assets: project.assets.len(),
        clips: project.clips.len(),
    }
}

fn print_status(view: &StatusView<'_>) {
    println!("Project: {}", view.project);
    println!("Root: {}", view.root);
    println!("Revision: {}", view.revision);
    println!(
        "Editor: {}",
        if view.editor.running {
            "running"
        } else {
            "not running"
        }
    );
    println!("Duration: {}", view.timeline.duration);
    println!("Playhead: {}", view.editor.playhead);
    println!(
        "Selection: {}",
        view.editor.selected_clip_id.unwrap_or("none")
    );
    println!("Assets: {}", view.assets);
    println!("Clips: {}", view.clips);
}

fn print_timeline(project: &Project) {
    let width = 72usize;
    let duration = project.duration_frames().max(1);
    for track in &project.tracks {
        let mut row = vec![' '; width];
        for clip in project
            .clips
            .iter()
            .filter(|clip| clip.track_id == track.id)
        {
            let start =
                ((clip.start_frame as f64 / duration as f64) * width as f64).floor() as usize;
            let end = ((clip.end_frame() as f64 / duration as f64) * width as f64).ceil() as usize;
            let end = end.clamp(start + 1, width);
            row[start] = '[';
            if end > start + 1 {
                row[end - 1] = ']';
                for cell in row.iter_mut().take(end - 1).skip(start + 1) {
                    *cell = '=';
                }
            }
        }
        println!("{:>3} |{}|", track.id, row.iter().collect::<String>());
    }
    let playhead =
        ((project.playhead_frame as f64 / duration as f64) * width as f64).round() as usize;
    println!(
        "    {}^ {}",
        " ".repeat(playhead.min(width)),
        format_timecode(project.playhead_frame, project.fps)
    );
}
