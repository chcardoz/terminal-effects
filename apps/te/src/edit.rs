use crate::model::{AssetKind, Clip, ClipTransform, FitMode, Project, TrackKind, new_id};
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

fn track_for_asset(project: &Project, kind: &AssetKind, requested: Option<&str>) -> Result<String> {
    let compatible = |track_kind: &TrackKind| {
        matches!(
            (kind, track_kind),
            (AssetKind::Video, TrackKind::Video) | (AssetKind::Audio, TrackKind::Audio)
        )
    };
    let track = if let Some(track_id) = requested {
        project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .with_context(|| format!("track not found: {track_id}"))?
    } else {
        project
            .tracks
            .iter()
            .find(|track| compatible(&track.kind))
            .context("project has no compatible track")?
    };
    if !compatible(&track.kind) {
        bail!("asset cannot be placed on {}", track.id);
    }
    Ok(track.id.clone())
}

fn add_to_project(
    project: &mut Project,
    asset_query: &str,
    track_id: Option<&str>,
    at_frame: i64,
    source_in_frame: i64,
    duration_frames: Option<i64>,
) -> Result<String> {
    if at_frame < 0 {
        bail!("clip start cannot be negative");
    }
    if source_in_frame < 0 {
        bail!("clip source in cannot be negative");
    }
    let asset = project.assets[project.resolve_asset_index(asset_query)?].clone();
    let track_id = track_for_asset(project, &asset.kind, track_id)?;
    let duration_frames = duration_frames.unwrap_or(asset.duration_frames - source_in_frame);
    if duration_frames <= 0 {
        bail!("clip duration must be positive");
    }
    if source_in_frame + duration_frames > asset.duration_frames + 1 {
        bail!("clip extends beyond the source media");
    }
    let clip_id = new_id("clip");
    project.clips.push(Clip {
        id: clip_id.clone(),
        asset_id: asset.id,
        track_id,
        start_frame: at_frame,
        duration_frames,
        source_in_frame,
        transform: ClipTransform::default(),
    });
    project.selected_clip_id = Some(clip_id.clone());
    Ok(clip_id)
}

pub fn add_clip(
    project_path: &Path,
    asset_query: &str,
    track_id: Option<&str>,
    at_frame: i64,
    source_in_frame: i64,
    duration_frames: Option<i64>,
) -> Result<EditReport> {
    mutate(project_path, "add", |project| {
        let created = add_to_project(
            project,
            asset_query,
            track_id,
            at_frame,
            source_in_frame,
            duration_frames,
        )?;
        Ok((Vec::new(), vec![created]))
    })
}

pub fn append_clip(
    project_path: &Path,
    asset_query: &str,
    track_id: Option<&str>,
    source_in_frame: i64,
    duration_frames: Option<i64>,
) -> Result<EditReport> {
    mutate(project_path, "append", |project| {
        let asset = project.assets[project.resolve_asset_index(asset_query)?].clone();
        let target_track = track_for_asset(project, &asset.kind, track_id)?;
        let at_frame = project
            .clips
            .iter()
            .filter(|clip| clip.track_id == target_track)
            .map(Clip::end_frame)
            .max()
            .unwrap_or(0);
        let created = add_to_project(
            project,
            &asset.id,
            Some(&target_track),
            at_frame,
            source_in_frame,
            duration_frames,
        )?;
        Ok((Vec::new(), vec![created]))
    })
}

pub fn duplicate_clip(
    project_path: &Path,
    clip_query: &str,
    track_id: Option<&str>,
    at_frame: i64,
    source_in_frame: Option<i64>,
    duration_frames: Option<i64>,
) -> Result<EditReport> {
    mutate(project_path, "duplicate", |project| {
        let source = project.clips[project.resolve_clip_index(clip_query)?].clone();
        let source_transform = source.transform.clone();
        let created = add_to_project(
            project,
            &source.asset_id,
            track_id.or(Some(source.track_id.as_str())),
            at_frame,
            source_in_frame.unwrap_or(source.source_in_frame),
            duration_frames.or(Some(source.duration_frames)),
        )?;
        let created_index = project.resolve_clip_index(&created)?;
        project.clips[created_index].transform = source_transform;
        Ok((vec![source.id], vec![created]))
    })
}

pub fn transform_clip(
    project_path: &Path,
    clip_query: &str,
    rotation_degrees: Option<i16>,
    fit: Option<FitMode>,
    position_x: Option<f64>,
    position_y: Option<f64>,
    reset: bool,
) -> Result<EditReport> {
    mutate(project_path, "transform", |project| {
        if reset
            && (rotation_degrees.is_some()
                || fit.is_some()
                || position_x.is_some()
                || position_y.is_some())
        {
            bail!("--reset cannot be combined with transform values");
        }
        if !reset
            && rotation_degrees.is_none()
            && fit.is_none()
            && position_x.is_none()
            && position_y.is_none()
        {
            bail!("provide --rotate, --fit, --position-x, --position-y, or --reset");
        }
        if let Some(rotation) = rotation_degrees
            && !matches!(rotation, 0 | 90 | 180 | 270)
        {
            bail!("rotation must be 0, 90, 180, or 270 degrees");
        }
        for (name, position) in [("position x", position_x), ("position y", position_y)] {
            if let Some(position) = position
                && (!position.is_finite() || !(0.0..=1.0).contains(&position))
            {
                bail!("{name} must be between 0 and 1");
            }
        }
        let index = project.resolve_clip_index(clip_query)?;
        let id = project.clips[index].id.clone();
        if reset {
            project.clips[index].transform = ClipTransform::default();
        } else {
            let transform = &mut project.clips[index].transform;
            if let Some(rotation) = rotation_degrees {
                transform.rotation_degrees = rotation;
            }
            if let Some(fit) = fit {
                transform.fit = fit;
            }
            if let Some(position) = position_x {
                transform.position_x = position;
            }
            if let Some(position) = position_y {
                transform.position_y = position;
            }
        }
        project.selected_clip_id = Some(id.clone());
        Ok((vec![id], Vec::new()))
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
                transform: clip.transform,
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

pub fn trim(
    project_path: &Path,
    clip_query: &str,
    start_frame: i64,
    duration_frames: i64,
    source_in_frame: i64,
) -> Result<EditReport> {
    mutate(project_path, "trim", |project| {
        if start_frame < 0 {
            bail!("clip start cannot be negative");
        }
        if duration_frames <= 0 {
            bail!("clip duration must be positive");
        }
        if source_in_frame < 0 {
            bail!("clip source in cannot be negative");
        }
        let index = project.resolve_clip_index(clip_query)?;
        let id = project.clips[index].id.clone();
        let asset = project
            .asset(&project.clips[index].asset_id)
            .context("clip asset missing")?;
        if source_in_frame + duration_frames > asset.duration_frames + 1 {
            bail!("trim extends beyond the source media");
        }
        project.clips[index].start_frame = start_frame;
        project.clips[index].duration_frames = duration_frames;
        project.clips[index].source_in_frame = source_in_frame;
        project.selected_clip_id = Some(id.clone());
        Ok((vec![id], Vec::new()))
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
            transform: ClipTransform::default(),
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

    #[test]
    fn trim_updates_all_timing_fields() {
        let (_temp, path) = fixture();
        trim(&path, "clip_a", 30, 120, 45).unwrap();
        let project = load(&path).unwrap();
        assert_eq!(project.clips[0].start_frame, 30);
        assert_eq!(project.clips[0].duration_frames, 120);
        assert_eq!(project.clips[0].source_in_frame, 45);
    }

    #[test]
    fn add_uses_source_remainder_and_supports_undo_redo() {
        let (_temp, path) = fixture();
        let report = add_clip(&path, "asset_a", None, 330, 60, None).unwrap();
        let created = report.created[0].clone();
        let project = load(&path).unwrap();
        let clip = project
            .clips
            .iter()
            .find(|clip| clip.id == created)
            .unwrap();
        assert_eq!(clip.track_id, "V1");
        assert_eq!(clip.start_frame, 330);
        assert_eq!(clip.source_in_frame, 60);
        assert_eq!(clip.duration_frames, 240);

        undo(&path).unwrap();
        assert_eq!(load(&path).unwrap().clips.len(), 1);
        redo(&path).unwrap();
        assert_eq!(load(&path).unwrap().clips.len(), 2);
    }

    #[test]
    fn append_places_clip_at_end_of_compatible_track() {
        let (_temp, path) = fixture();
        let report = append_clip(&path, "asset_a", None, 30, Some(90)).unwrap();
        let project = load(&path).unwrap();
        let clip = project
            .clips
            .iter()
            .find(|clip| clip.id == report.created[0])
            .unwrap();
        assert_eq!(clip.start_frame, 300);
        assert_eq!(clip.source_in_frame, 30);
        assert_eq!(clip.duration_frames, 90);
    }

    #[test]
    fn duplicate_can_override_source_range() {
        let (_temp, path) = fixture();
        transform_clip(
            &path,
            "clip_a",
            Some(90),
            Some(FitMode::Cover),
            None,
            None,
            false,
        )
        .unwrap();
        let report = duplicate_clip(&path, "clip_a", None, 400, Some(120), Some(60)).unwrap();
        let project = load(&path).unwrap();
        let clip = project
            .clips
            .iter()
            .find(|clip| clip.id == report.created[0])
            .unwrap();
        assert_eq!(report.affected, vec!["clip_a"]);
        assert_eq!(clip.asset_id, "asset_a");
        assert_eq!(clip.track_id, "V1");
        assert_eq!(clip.start_frame, 400);
        assert_eq!(clip.source_in_frame, 120);
        assert_eq!(clip.duration_frames, 60);
        assert_eq!(clip.transform.rotation_degrees, 90);
        assert_eq!(clip.transform.fit, FitMode::Cover);
    }

    #[test]
    fn add_rejects_ranges_beyond_source_media() {
        let (_temp, path) = fixture();
        let error = add_clip(&path, "asset_a", None, 0, 250, Some(60)).unwrap_err();
        assert!(error.to_string().contains("beyond the source media"));
        assert_eq!(load(&path).unwrap().clips.len(), 1);
    }

    #[test]
    fn transform_is_non_destructive_and_undoable() {
        let (_temp, path) = fixture();
        transform_clip(
            &path,
            "clip_a",
            Some(90),
            Some(FitMode::Cover),
            Some(0.25),
            Some(0.75),
            false,
        )
        .unwrap();
        let project = load(&path).unwrap();
        assert_eq!(project.clips[0].source_in_frame, 0);
        assert_eq!(project.clips[0].duration_frames, 300);
        assert_eq!(project.clips[0].transform.rotation_degrees, 90);
        assert_eq!(project.clips[0].transform.fit, FitMode::Cover);
        assert_eq!(project.clips[0].transform.position_x, 0.25);
        assert_eq!(project.clips[0].transform.position_y, 0.75);

        undo(&path).unwrap();
        assert_eq!(
            load(&path).unwrap().clips[0].transform,
            ClipTransform::default()
        );
    }

    #[test]
    fn transform_rejects_invalid_values_without_history_entry() {
        let (_temp, path) = fixture();
        assert!(transform_clip(&path, "clip_a", Some(45), None, None, None, false).is_err());
        assert!(transform_clip(&path, "clip_a", None, None, Some(1.1), None, false).is_err());
        assert_eq!(load(&path).unwrap().revision, 0);
    }
}
