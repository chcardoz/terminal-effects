use crate::model::{AssetKind, Clip, Project, TrackKind, new_id};
use crate::project::{load, save};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditReport {
    pub ok: bool,
    pub operation: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub affected: Vec<String>,
    pub created: Vec<String>,
    pub undoable: bool,
}

fn history_dir(project_path: &Path, stack: &str) -> Result<PathBuf> {
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    Ok(root.join(".te/history").join(stack))
}

fn snapshot_name() -> Result<String> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!("{nanos:032}.teproj"))
}

fn push_snapshot(project_path: &Path, stack: &str, project: &Project) -> Result<PathBuf> {
    let dir = history_dir(project_path, stack)?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(snapshot_name()?);
    save(&path, project)?;
    Ok(path)
}

fn clear_stack(project_path: &Path, stack: &str) -> Result<()> {
    let dir = history_dir(project_path, stack)?;
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "teproj") {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn pop_snapshot(project_path: &Path, stack: &str) -> Result<Option<(PathBuf, Project)>> {
    let dir = history_dir(project_path, stack)?;
    if !dir.exists() {
        return Ok(None);
    }
    let mut entries = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "teproj"))
        .collect::<Vec<_>>();
    entries.sort();
    let Some(path) = entries.pop() else {
        return Ok(None);
    };
    let project = load(&path)?;
    Ok(Some((path, project)))
}

fn mutate<F>(project_path: &Path, operation: &str, apply: F) -> Result<EditReport>
where
    F: FnOnce(&mut Project) -> Result<(Vec<String>, Vec<String>)>,
{
    let mut project = load(project_path)?;
    let before = project.revision;
    let original = project.clone();
    let (affected, created) = apply(&mut project)?;
    project.validate()?;
    push_snapshot(project_path, "undo", &original)?;
    clear_stack(project_path, "redo")?;
    project.revision += 1;
    save(project_path, &project)?;
    Ok(EditReport {
        ok: true,
        operation: operation.into(),
        revision_before: before,
        revision_after: project.revision,
        affected,
        created,
        undoable: true,
    })
}

pub fn split(project_path: &Path, clip_query: &str, at_frame: i64) -> Result<EditReport> {
    mutate(project_path, "split", |project| {
        let index = project.resolve_clip_index(clip_query)?;
        let clip = project.clips[index].clone();
        if at_frame <= clip.start_frame || at_frame >= clip.end_frame() {
            bail!(
                "split point must be inside {} ({}f..{}f)",
                clip.id,
                clip.start_frame,
                clip.end_frame()
            );
        }
        let offset = at_frame - clip.start_frame;
        let right_id = new_id("clip");
        project.clips[index].duration_frames = offset;
        project.clips.insert(
            index + 1,
            Clip {
                id: right_id.clone(),
                asset_id: clip.asset_id,
                track_id: clip.track_id,
                start_frame: at_frame,
                duration_frames: clip.duration_frames - offset,
                source_in_frame: clip.source_in_frame + offset,
            },
        );
        project.selected_clip_id = Some(right_id.clone());
        Ok((vec![clip.id], vec![right_id]))
    })
}

pub fn move_clip(
    project_path: &Path,
    clip_query: &str,
    track_id: Option<&str>,
    at_frame: i64,
) -> Result<EditReport> {
    mutate(project_path, "move", |project| {
        if at_frame < 0 {
            bail!("clip start cannot be negative");
        }
        let index = project.resolve_clip_index(clip_query)?;
        let id = project.clips[index].id.clone();
        if let Some(track_id) = track_id {
            let track = project
                .tracks
                .iter()
                .find(|track| track.id == track_id)
                .with_context(|| format!("track not found: {track_id}"))?;
            let asset = project
                .asset(&project.clips[index].asset_id)
                .context("clip asset missing")?;
            let compatible = matches!(
                (&asset.kind, &track.kind),
                (AssetKind::Video, TrackKind::Video) | (AssetKind::Audio, TrackKind::Audio)
            );
            if !compatible {
                bail!("{} cannot be placed on {}", asset.name, track_id);
            }
            project.clips[index].track_id = track_id.into();
        }
        project.clips[index].start_frame = at_frame;
        project.selected_clip_id = Some(id.clone());
        Ok((vec![id], Vec::new()))
    })
}

pub fn remove(project_path: &Path, clip_query: &str) -> Result<EditReport> {
    mutate(project_path, "remove", |project| {
        let index = project.resolve_clip_index(clip_query)?;
        let removed = project.clips.remove(index).id;
        project.selected_clip_id = project
            .clips
            .get(index)
            .or_else(|| project.clips.last())
            .map(|clip| clip.id.clone());
        Ok((vec![removed], Vec::new()))
    })
}

pub fn undo(project_path: &Path) -> Result<EditReport> {
    restore(project_path, "undo", "redo", "undo")
}

pub fn redo(project_path: &Path) -> Result<EditReport> {
    restore(project_path, "redo", "undo", "redo")
}

fn restore(project_path: &Path, from: &str, to: &str, operation: &str) -> Result<EditReport> {
    let current = load(project_path)?;
    let before = current.revision;
    let Some((snapshot_path, mut restored)) = pop_snapshot(project_path, from)? else {
        bail!("nothing to {operation}");
    };
    push_snapshot(project_path, to, &current)?;
    fs::remove_file(snapshot_path)?;
    restored.revision = before + 1;
    save(project_path, &restored)?;
    Ok(EditReport {
        ok: true,
        operation: operation.into(),
        revision_before: before,
        revision_after: restored.revision,
        affected: restored.clips.iter().map(|clip| clip.id.clone()).collect(),
        created: Vec::new(),
        undoable: operation != "undo",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Asset, Fps};
    use crate::project::project_path;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join(".te/history/undo")).unwrap();
        fs::create_dir_all(temp.path().join(".te/history/redo")).unwrap();
        let mut project = Project::empty("test".into());
        project.fps = Fps::default();
        project.assets.push(Asset {
            id: "asset_a".into(),
            name: "a.mp4".into(),
            path: "a.mp4".into(),
            kind: AssetKind::Video,
            duration_frames: 300,
            width: 1920,
            height: 1080,
            has_audio: true,
        });
        project.clips.push(Clip {
            id: "clip_a".into(),
            asset_id: "asset_a".into(),
            track_id: "V1".into(),
            start_frame: 0,
            duration_frames: 300,
            source_in_frame: 0,
        });
        project.selected_clip_id = Some("clip_a".into());
        let path = project_path(temp.path());
        save(&path, &project).unwrap();
        (temp, path)
    }

    #[test]
    fn split_preserves_source_duration() {
        let (_temp, path) = fixture();
        let report = split(&path, "clip_a", 120).unwrap();
        let project = load(&path).unwrap();
        assert_eq!(report.revision_after, 1);
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.clips[0].duration_frames, 120);
        assert_eq!(project.clips[1].source_in_frame, 120);
        assert_eq!(project.clips[1].duration_frames, 180);
    }

    #[test]
    fn undo_restores_split() {
        let (_temp, path) = fixture();
        split(&path, "clip_a", 120).unwrap();
        undo(&path).unwrap();
        let project = load(&path).unwrap();
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.clips[0].duration_frames, 300);
    }
}
