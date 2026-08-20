use crate::edit;
use crate::media::{export, frame_at};
use crate::model::Project;
use crate::project::{load, save};
use crate::render::Canvas;
use anyhow::{Context, Result, bail};
use image::RgbaImage;
use pixel_core::wrapper::Wrapper;
use pixel_core::{Event, Key, KeyEvent, KeyKind, MouseButton, MouseKind, SessionEnv, Terminal};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{IsTerminal, stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const VIEW_WIDTH: u32 = 1200;
const VIEW_HEIGHT: u32 = 760;
const TIMELINE_X: i32 = 126;
const TIMELINE_Y: i32 = 508;
const TIMELINE_WIDTH: i32 = 1074;
const TRACK_HEIGHT: i32 = 78;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub pid: u32,
    pub project: String,
}

struct TerminalGuard {
    session_path: PathBuf,
    terminal: Terminal,
}

impl TerminalGuard {
    fn enter(root: &Path, project_path: &Path) -> Result<Self> {
        if !std::io::stdin().is_terminal() || !stdout().is_terminal() {
            bail!("`te .` needs an interactive terminal; agent commands work without one");
        }
        let wrapper = if std::env::var_os("TMUX").is_some() {
            Wrapper::Tmux
        } else {
            Wrapper::None
        };
        let mut terminal = Terminal::new(wrapper, SessionEnv::of_process())?;
        terminal.watch_resize()?;
        let session_path = root.join(".te/session.json");
        fs::create_dir_all(root.join(".te"))?;
        fs::write(
            &session_path,
            serde_json::to_vec_pretty(&SessionInfo {
                pid: std::process::id(),
                project: project_path.display().to_string(),
            })?,
        )?;
        Ok(Self {
            session_path,
            terminal,
        })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.session_path);
    }
}

pub fn session_running(root: &Path) -> bool {
    let Ok(bytes) = fs::read(root.join(".te/session.json")) else {
        return false;
    };
    let Ok(session) = serde_json::from_slice::<SessionInfo>(&bytes) else {
        return false;
    };
    unsafe { libc::kill(session.pid as i32, 0) == 0 }
}

pub fn snapshot(project_path: &Path, output: Option<&Path>) -> Result<PathBuf> {
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let project = load(project_path)?;
    let preview = frame_at(root, &project, project.playhead_frame, None)
        .and_then(|path| Ok(image::open(path)?.to_rgba8()))
        .ok();
    let canvas = crate::ui::editor_canvas(
        &project,
        preview.as_ref(),
        "Agent snapshot of the live project",
        false,
    );
    let output = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(".te/cache/editor.png"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    image::DynamicImage::ImageRgba8(canvas.rgba_image()).save(&output)?;
    Ok(output)
}

pub fn run(project_path: &Path) -> Result<()> {
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let mut guard = TerminalGuard::enter(root, project_path)?;
    let mut project = load(project_path)?;
    let mut preview: Option<(i64, RgbaImage)> = None;
    let mut status = String::from("Ready");
    let mut playing = false;
    let mut last_tick = Instant::now();
    let mut modified = file_modified(project_path);
    let mut needs_redraw = true;

    loop {
        if file_modified(project_path) != modified {
            project = load(project_path)?;
            modified = file_modified(project_path);
            preview = None;
            status = "Project updated".into();
            needs_redraw = true;
        }
        if playing && last_tick.elapsed() >= Duration::from_millis(120) {
            let step = (project.fps.as_f64() / 8.0).round().max(1.0) as i64;
            project.playhead_frame += step;
            if project.playhead_frame >= project.duration_frames() {
                project.playhead_frame = project.duration_frames();
                playing = false;
            }
            preview = None;
            last_tick = Instant::now();
            needs_redraw = true;
        }
        if preview
            .as_ref()
            .is_none_or(|(frame, _)| *frame != project.playhead_frame)
        {
            match frame_at(root, &project, project.playhead_frame, None)
                .and_then(|path| Ok(image::open(path)?.to_rgba8()))
            {
                Ok(image) => {
                    preview = Some((project.playhead_frame, image));
                    needs_redraw = true;
                }
                Err(error) => {
                    status = format!("Preview: {error}");
                    needs_redraw = true;
                }
            }
        }
        if needs_redraw {
            let canvas = editor_canvas(
                &mut guard.terminal,
                &project,
                preview.as_ref().map(|(_, image)| image),
                &status,
                playing,
            )?;
            present(&mut guard.terminal, &canvas)?;
            needs_redraw = false;
        }

        let Some(event) = guard.terminal.poll_event(Some(Duration::from_millis(80)))? else {
            continue;
        };
        match event {
            Event::Key(key) if key.kind != KeyKind::Release => {
                if handle_key(
                    &mut guard.terminal,
                    project_path,
                    &mut project,
                    key,
                    &mut status,
                    &mut playing,
                )? {
                    break;
                }
                modified = file_modified(project_path);
                preview = None;
                needs_redraw = true;
            }
            Event::Mouse(mouse)
                if mouse.kind == MouseKind::Down && mouse.button == MouseButton::Left =>
            {
                let (width, height) = viewport_pixels(&mut guard.terminal)?;
                let x = mouse.x as f64 / width.max(1) as f64 * VIEW_WIDTH as f64;
                let y = mouse.y as f64 / height.max(1) as f64 * VIEW_HEIGHT as f64;
                if y >= TIMELINE_Y as f64 && y <= (TIMELINE_Y + TRACK_HEIGHT * 2) as f64 {
                    let duration = project.duration_frames().max(1);
                    let position =
                        ((x - TIMELINE_X as f64) / TIMELINE_WIDTH as f64).clamp(0.0, 1.0);
                    project.playhead_frame = (position * duration as f64).round() as i64;
                    let track = if y < (TIMELINE_Y + TRACK_HEIGHT) as f64 {
                        "V1"
                    } else {
                        "A1"
                    };
                    let direct = project.clips.iter().rfind(|clip| {
                        clip.track_id == track
                            && clip.start_frame <= project.playhead_frame
                            && project.playhead_frame < clip.end_frame()
                    });
                    let linked_audio = (track == "A1").then(|| {
                        project.clips.iter().rfind(|clip| {
                            clip.start_frame <= project.playhead_frame
                                && project.playhead_frame < clip.end_frame()
                                && project
                                    .asset(&clip.asset_id)
                                    .is_some_and(|asset| asset.has_audio)
                        })
                    });
                    project.selected_clip_id = direct
                        .or_else(|| linked_audio.flatten())
                        .map(|clip| clip.id.clone());
                    save(project_path, &project)?;
                    status = "Playhead moved".into();
                    modified = file_modified(project_path);
                    preview = None;
                    needs_redraw = true;
                }
            }
            Event::WindowSize(_) => {
                guard.terminal.forget_cell_size();
                needs_redraw = true;
            }
            _ => {}
        }
    }
    save(project_path, &project)?;
    Ok(())
}

fn handle_key(
    terminal: &mut Terminal,
    project_path: &Path,
    project: &mut Project,
    key: KeyEvent,
    status: &mut String,
    playing: &mut bool,
) -> Result<bool> {
    if key.key == Key::Char('q') || key.key == Key::Escape {
        return Ok(true);
    }
    if key.mods.ctrl && key.key == Key::Char('z') {
        *status = report(edit::undo(project_path));
        *project = load(project_path)?;
        return Ok(false);
    }
    if key.mods.ctrl && key.key == Key::Char('y') {
        *status = report(edit::redo(project_path));
        *project = load(project_path)?;
        return Ok(false);
    }
    match key.key {
        Key::Left => {
            let step = if key.mods.shift {
                project.fps.seconds_to_frames(1.0)
            } else {
                1
            };
            project.playhead_frame = (project.playhead_frame - step).max(0);
            save(project_path, project)?;
        }
        Key::Right => {
            let step = if key.mods.shift {
                project.fps.seconds_to_frames(1.0)
            } else {
                1
            };
            project.playhead_frame = (project.playhead_frame + step).min(project.duration_frames());
            save(project_path, project)?;
        }
        Key::Home => {
            project.playhead_frame = 0;
            save(project_path, project)?;
        }
        Key::End => {
            project.playhead_frame = project.duration_frames();
            save(project_path, project)?;
        }
        Key::Tab => {
            if !project.clips.is_empty() {
                let current = project
                    .selected_clip_id
                    .as_ref()
                    .and_then(|id| project.clips.iter().position(|clip| &clip.id == id))
                    .unwrap_or(project.clips.len() - 1);
                project.selected_clip_id = Some(
                    project.clips[(current + 1) % project.clips.len()]
                        .id
                        .clone(),
                );
                save(project_path, project)?;
            }
        }
        Key::Char(' ') => {
            *playing = !*playing;
            *status = if *playing {
                "Playing preview"
            } else {
                "Paused"
            }
            .into();
        }
        Key::Char('s') => {
            if let Some(id) = project.selected_clip_id.clone() {
                *status = report(edit::split(project_path, &id, project.playhead_frame));
                *project = load(project_path)?;
            } else {
                *status = "Select a clip before splitting".into();
            }
        }
        Key::Delete | Key::Backspace => {
            if let Some(id) = project.selected_clip_id.clone() {
                *status = report(edit::remove(project_path, &id));
                *project = load(project_path)?;
            }
        }
        Key::Char('e') => {
            let root = project_path
                .parent()
                .context("project file has no parent")?;
            let output = root.join(".te/exports/export.mp4");
            *status = "Exporting...".into();
            let canvas = editor_canvas(terminal, project, None, status, false)?;
            present(terminal, &canvas)?;
            export(root, project, &output)?;
            *status = format!("Exported {}", output.display());
        }
        _ => {}
    }
    Ok(false)
}

fn report(result: Result<edit::EditReport>) -> String {
    match result {
        Ok(report) => format!("{} -> revision {}", report.operation, report.revision_after),
        Err(error) => error.to_string(),
    }
}

fn file_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn editor_canvas(
    terminal: &mut Terminal,
    project: &Project,
    preview: Option<&RgbaImage>,
    status: &str,
    playing: bool,
) -> Result<Canvas> {
    let (width, height) = viewport_pixels(terminal)?;
    Ok(crate::ui::editor_canvas_at_size(
        project, preview, status, playing, width, height,
    ))
}

fn viewport_pixels(terminal: &mut Terminal) -> Result<(u32, u32)> {
    let window = terminal.size()?;
    let cell = terminal.cell_size()?.unwrap_or((16, 32));
    let width = if window.width_px > 0 {
        window.width_px.min(window.cols.max(1) * cell.0)
    } else {
        window.cols.max(1) * cell.0
    };
    let height = if window.height_px > 0 {
        window.height_px.min(window.rows.max(1) * cell.1)
    } else {
        window.rows.max(1) * cell.1
    };
    Ok((width.max(1), height.max(1)))
}

fn present(terminal: &mut Terminal, canvas: &Canvas) -> Result<()> {
    terminal.draw(canvas.core())?;
    Ok(())
}
