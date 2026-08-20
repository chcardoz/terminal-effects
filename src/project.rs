use crate::media::{MediaProbe, is_media_path, probe_media};
use crate::model::{Asset, AssetKind, Clip, PROJECT_FILE, Project, new_id};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub fn project_path(root: &Path) -> PathBuf {
    root.join(PROJECT_FILE)
}

pub fn find_project(start: &Path) -> Result<PathBuf> {
    let start = start
        .canonicalize()
        .with_context(|| format!("cannot access {}", start.display()))?;
    let mut current = if start.is_file() {
        if start.file_name().is_some_and(|name| name == PROJECT_FILE) {
            return Ok(start);
        }
        start.parent().context("file has no parent")?.to_path_buf()
    } else {
        start
    };
    loop {
        let candidate = project_path(&current);
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !current.pop() {
            break;
        }
    }
    bail!("no {PROJECT_FILE} found; run `te .` in the directory you want to edit")
}

pub fn load(path: &Path) -> Result<Project> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let project: Project = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid project file {}", path.display()))?;
    project.validate()?;
    Ok(project)
}

pub fn save(path: &Path, project: &Project) -> Result<()> {
    project.validate()?;
    let bytes = serde_json::to_vec_pretty(project)?;
    let temp = path.with_extension(format!("teproj.tmp-{}", std::process::id()));
    fs::write(&temp, bytes).with_context(|| format!("cannot write {}", temp.display()))?;
    fs::rename(&temp, path).with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

pub fn ensure_layout(root: &Path) -> Result<()> {
    for relative in [
        ".te/cache/frames",
        ".te/cache/filmstrips",
        ".te/cache/thumbnails",
        ".te/exports",
        ".te/history/undo",
        ".te/history/redo",
    ] {
        fs::create_dir_all(root.join(relative))?;
    }
    Ok(())
}

pub fn open_or_create(path: &Path) -> Result<PathBuf> {
    let path = if path.exists() {
        path.canonicalize()?
    } else {
        fs::create_dir_all(path)?;
        path.canonicalize()?
    };
    if path.is_file() {
        return find_project(&path);
    }
    let existing = project_path(&path);
    if existing.is_file() {
        return Ok(existing);
    }
    ensure_layout(&path)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("untitled")
        .to_string();
    let mut project = Project::empty(name);
    let mut media = fs::read_dir(&path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry| entry.is_file() && is_media_path(entry))
        .collect::<Vec<_>>();
    media.sort();
    for media_path in media {
        import_one(&path, &mut project, &media_path)?;
    }
    save(&existing, &project)?;
    Ok(existing)
}

pub fn import_paths(path: &Path, project: &mut Project, files: &[PathBuf]) -> Result<Vec<String>> {
    let root = path.parent().context("project file has no parent")?;
    let mut imported = Vec::new();
    for file in files {
        let canonical = file
            .canonicalize()
            .with_context(|| format!("cannot access {}", file.display()))?;
        if !is_media_path(&canonical) {
            bail!("unsupported media file: {}", canonical.display());
        }
        if let Some(asset) = project
            .assets
            .iter()
            .find(|asset| resolve_asset_path(root, asset) == canonical)
        {
            imported.push(asset.id.clone());
            continue;
        }
        imported.push(import_one(root, project, &canonical)?);
    }
    project.revision += 1;
    save(path, project)?;
    Ok(imported)
}

fn import_one(root: &Path, project: &mut Project, file: &Path) -> Result<String> {
    let MediaProbe {
        duration,
        width,
        height,
        has_video,
        has_audio,
    } = probe_media(file)?;
    if project.assets.is_empty() && has_video && width > 0 && height > 0 {
        project.width = width;
        project.height = height;
    }
    let relative = file.strip_prefix(root).unwrap_or(file);
    let asset_id = new_id("asset");
    let duration_frames = project.fps.seconds_to_frames(duration).max(1);
    let kind = if has_video {
        AssetKind::Video
    } else {
        AssetKind::Audio
    };
    project.assets.push(Asset {
        id: asset_id.clone(),
        name: file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("media")
            .to_string(),
        path: relative.to_string_lossy().to_string(),
        kind: kind.clone(),
        duration_frames,
        width,
        height,
        has_audio,
    });
    let track_id = if kind == AssetKind::Video { "V1" } else { "A1" };
    let start_frame = project
        .clips
        .iter()
        .filter(|clip| clip.track_id == track_id)
        .map(Clip::end_frame)
        .max()
        .unwrap_or(0);
    let clip_id = new_id("clip");
    project.clips.push(Clip {
        id: clip_id.clone(),
        asset_id: asset_id.clone(),
        track_id: track_id.into(),
        start_frame,
        duration_frames,
        source_in_frame: 0,
    });
    if project.selected_clip_id.is_none() {
        project.selected_clip_id = Some(clip_id);
    }
    Ok(asset_id)
}

pub fn resolve_asset_path(root: &Path, asset: &Asset) -> PathBuf {
    let path = Path::new(&asset.path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_project_from_child_directory() {
        let temp = tempfile::tempdir().unwrap();
        let project = Project::empty("test".into());
        save(&project_path(temp.path()), &project).unwrap();
        let child = temp.path().join("a/b");
        fs::create_dir_all(&child).unwrap();
        assert_eq!(
            find_project(&child).unwrap(),
            project_path(temp.path()).canonicalize().unwrap()
        );
    }
}
