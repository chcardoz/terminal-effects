use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const PROJECT_FILE: &str = "project.teproj";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fps {
    pub numerator: u32,
    pub denominator: u32,
}

impl Default for Fps {
    fn default() -> Self {
        Self {
            numerator: 30,
            denominator: 1,
        }
    }
}

impl Fps {
    pub fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator.max(1) as f64
    }

    pub fn seconds_to_frames(self, seconds: f64) -> i64 {
        (seconds * self.as_f64()).round() as i64
    }

    pub fn frames_to_seconds(self, frames: i64) -> f64 {
        frames as f64 / self.as_f64()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: AssetKind,
    pub duration_frames: i64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum FitMode {
    #[default]
    Contain,
    Cover,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClipTransform {
    pub rotation_degrees: i16,
    pub fit: FitMode,
    pub position_x: f64,
    pub position_y: f64,
}

impl Default for ClipTransform {
    fn default() -> Self {
        Self {
            rotation_degrees: 0,
            fit: FitMode::Contain,
            position_x: 0.5,
            position_y: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub kind: TrackKind,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub asset_id: String,
    pub track_id: String,
    pub start_frame: i64,
    pub duration_frames: i64,
    pub source_in_frame: i64,
    #[serde(default)]
    pub transform: ClipTransform,
}

impl Clip {
    pub fn end_frame(&self) -> i64 {
        self.start_frame + self.duration_frames
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    pub revision: u64,
    pub fps: Fps,
    pub width: u32,
    pub height: u32,
    pub playhead_frame: i64,
    pub selected_clip_id: Option<String>,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
}

impl Project {
    pub fn empty(name: String) -> Self {
        Self {
            schema_version: 1,
            name,
            revision: 0,
            fps: Fps::default(),
            width: 1920,
            height: 1080,
            playhead_frame: 0,
            selected_clip_id: None,
            assets: Vec::new(),
            tracks: vec![
                Track {
                    id: "V1".into(),
                    kind: TrackKind::Video,
                    name: "Video 1".into(),
                },
                Track {
                    id: "A1".into(),
                    kind: TrackKind::Audio,
                    name: "Audio 1".into(),
                },
            ],
            clips: Vec::new(),
        }
    }

    pub fn duration_frames(&self) -> i64 {
        self.clips.iter().map(Clip::end_frame).max().unwrap_or(0)
    }

    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn resolve_asset_index(&self, query: &str) -> Result<usize> {
        if let Some(index) = self.assets.iter().position(|asset| asset.id == query) {
            return Ok(index);
        }
        let matches = self
            .assets
            .iter()
            .enumerate()
            .filter(|(_, asset)| asset.id.starts_with(query))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [index] => Ok(*index),
            [] => bail!("asset not found: {query}"),
            _ => bail!("asset prefix is ambiguous: {query}"),
        }
    }

    pub fn resolve_clip_index(&self, query: &str) -> Result<usize> {
        if let Some(index) = self.clips.iter().position(|clip| clip.id == query) {
            return Ok(index);
        }
        let matches = self
            .clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.id.starts_with(query))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [index] => Ok(*index),
            [] => bail!("clip not found: {query}"),
            _ => bail!("clip prefix is ambiguous: {query}"),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("unsupported project schema version {}", self.schema_version);
        }
        if self.fps.numerator == 0 || self.fps.denominator == 0 {
            bail!("project fps must be positive");
        }
        let assets = self
            .assets
            .iter()
            .map(|asset| asset.id.as_str())
            .collect::<HashSet<_>>();
        let tracks = self
            .tracks
            .iter()
            .map(|track| track.id.as_str())
            .collect::<HashSet<_>>();
        let mut clips = HashSet::new();
        for clip in &self.clips {
            if !clips.insert(clip.id.as_str()) {
                bail!("duplicate clip id: {}", clip.id);
            }
            if !assets.contains(clip.asset_id.as_str()) {
                bail!("clip {} refers to missing asset {}", clip.id, clip.asset_id);
            }
            if !tracks.contains(clip.track_id.as_str()) {
                bail!("clip {} refers to missing track {}", clip.id, clip.track_id);
            }
            if clip.start_frame < 0 || clip.duration_frames <= 0 || clip.source_in_frame < 0 {
                bail!("clip {} has invalid timing", clip.id);
            }
            if !matches!(clip.transform.rotation_degrees, 0 | 90 | 180 | 270) {
                bail!("clip {} has invalid rotation", clip.id);
            }
            if !clip.transform.position_x.is_finite()
                || !clip.transform.position_y.is_finite()
                || !(0.0..=1.0).contains(&clip.transform.position_x)
                || !(0.0..=1.0).contains(&clip.transform.position_y)
            {
                bail!("clip {} has invalid transform position", clip.id);
            }
            let asset = self.asset(&clip.asset_id).expect("asset checked above");
            if clip.source_in_frame + clip.duration_frames > asset.duration_frames + 1 {
                bail!("clip {} extends beyond its source", clip.id);
            }
        }
        if let Some(selected) = &self.selected_clip_id
            && !clips.contains(selected.as_str())
        {
            bail!("selected clip does not exist: {selected}");
        }
        Ok(())
    }
}

pub fn new_id(prefix: &str) -> String {
    let raw = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &raw[..10])
}

pub fn format_timecode(frames: i64, fps: Fps) -> String {
    let total_ms = (fps.frames_to_seconds(frames.max(0)) * 1000.0).round() as i64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms / 60_000) % 60;
    let seconds = (total_ms / 1000) % 60;
    let millis = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

pub fn parse_time(value: &str, fps: Fps) -> Result<i64> {
    let value = value.trim();
    if let Some(frames) = value.strip_suffix('f') {
        return Ok(frames.trim().parse()?);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return Ok(fps.seconds_to_frames(seconds.trim().parse()?));
    }
    if value.contains(':') {
        let fields = value.split(':').collect::<Vec<_>>();
        let seconds = match fields.as_slice() {
            [minutes, seconds] => minutes.parse::<f64>()? * 60.0 + seconds.parse::<f64>()?,
            [hours, minutes, seconds] => {
                hours.parse::<f64>()? * 3600.0
                    + minutes.parse::<f64>()? * 60.0
                    + seconds.parse::<f64>()?
            }
            _ => bail!("invalid time: {value}"),
        };
        return Ok(fps.seconds_to_frames(seconds));
    }
    Ok(fps.seconds_to_frames(value.parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_friendly_times() {
        let fps = Fps::default();
        assert_eq!(parse_time("45f", fps).unwrap(), 45);
        assert_eq!(parse_time("1.5s", fps).unwrap(), 45);
        assert_eq!(parse_time("00:01.500", fps).unwrap(), 45);
        assert_eq!(parse_time("00:00:01.500", fps).unwrap(), 45);
    }

    #[test]
    fn formats_timecode() {
        assert_eq!(format_timecode(45, Fps::default()), "00:00:01.500");
    }

    #[test]
    fn resolves_unambiguous_asset_prefixes() {
        let mut project = Project::empty("test".into());
        project.assets.push(Asset {
            id: "asset_abcdef".into(),
            name: "a.mp4".into(),
            path: "a.mp4".into(),
            kind: AssetKind::Video,
            duration_frames: 30,
            width: 1920,
            height: 1080,
            has_audio: true,
        });
        assert_eq!(project.resolve_asset_index("asset_a").unwrap(), 0);
        assert!(project.resolve_asset_index("missing").is_err());
    }
}
