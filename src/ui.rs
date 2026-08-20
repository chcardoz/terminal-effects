use crate::icons::{self, Icon};
use crate::model::{Asset, AssetKind, Clip, Project, format_timecode};
use crate::render::{Canvas, Color};
use image::RgbaImage;

const WIDTH: i32 = 1200;
const HEIGHT: i32 = 760;
const TOP_HEIGHT: i32 = 44;
const MEDIA_WIDTH: i32 = 278;
const INSPECTOR_X: i32 = 956;
const WORKSPACE_BOTTOM: i32 = 442;
const TIMELINE_X: i32 = 126;
const TIMELINE_Y: i32 = 508;
const TIMELINE_WIDTH: i32 = WIDTH - TIMELINE_X;
const TRACK_HEIGHT: i32 = 78;

// Neutral editor chrome with one functional accent. Saturated colors are kept
// inside media/timeline content where they communicate track type or state.
const BACKGROUND: Color = [13, 14, 16, 255];
const CHROME: Color = [18, 19, 22, 255];
const PANEL: Color = [21, 22, 25, 255];
const PANEL_RAISED: Color = [25, 27, 30, 255];
const SURFACE: Color = [30, 32, 36, 255];
const FIELD: Color = [16, 17, 20, 255];
const DIVIDER: Color = [44, 47, 52, 255];
const DIVIDER_SOFT: Color = [34, 37, 41, 255];
const TEXT: Color = [232, 235, 239, 255];
const TEXT_SECONDARY: Color = [166, 172, 181, 255];
const TEXT_MUTED: Color = [103, 110, 121, 255];
const ACCENT: Color = [70, 196, 170, 255];
const VIDEO_FILL: Color = [31, 61, 58, 255];
const VIDEO_TOP: Color = [37, 78, 72, 255];
const AUDIO: Color = [217, 157, 70, 255];
const AUDIO_FILL: Color = [70, 53, 31, 255];
const PLAYHEAD: Color = [255, 90, 94, 255];

pub fn editor_canvas(
    project: &Project,
    preview: Option<&RgbaImage>,
    status: &str,
    playing: bool,
) -> Canvas {
    editor_canvas_at_size(project, preview, status, playing, 2400, 1520)
}

pub fn editor_canvas_at_size(
    project: &Project,
    preview: Option<&RgbaImage>,
    status: &str,
    playing: bool,
    physical_width: u32,
    physical_height: u32,
) -> Canvas {
    let mut canvas = Canvas::new_viewport(
        WIDTH as u32,
        HEIGHT as u32,
        physical_width,
        physical_height,
        BACKGROUND,
    );
    draw_top_bar(&mut canvas, project);
    draw_media_browser(&mut canvas, project, preview);
    draw_program_monitor(&mut canvas, project, preview, playing);
    draw_inspector(&mut canvas, project);
    draw_timeline(&mut canvas, project);
    draw_status_bar(&mut canvas, status);
    canvas
}

fn draw_top_bar(canvas: &mut Canvas, project: &Project) {
    canvas.rect(0, 0, WIDTH, TOP_HEIGHT, CHROME);
    canvas.rect(0, TOP_HEIGHT - 1, WIDTH, 1, DIVIDER);

    icons::draw(canvas, Icon::Film, 14, 12, 18, ACCENT);
    canvas.text_sized(40, 14, "TERMINAL EFFECTS", 12.0, TEXT);
    canvas.rect(174, 10, 1, 24, DIVIDER_SOFT);
    canvas.text_sized(190, 14, &truncate(&project.name, 34), 12.0, TEXT_SECONDARY);

    canvas.text_sized(
        997,
        16,
        &format!("REV {:02}", project.revision),
        9.5,
        TEXT_MUTED,
    );
    canvas.rect(1073, 7, 112, 30, ACCENT);
    icons::draw(canvas, Icon::Download, 1086, 14, 16, [13, 27, 24, 255]);
    canvas.text_sized(1110, 14, "Export", 11.5, [13, 27, 24, 255]);
}

fn draw_media_browser(canvas: &mut Canvas, project: &Project, preview: Option<&RgbaImage>) {
    canvas.rect(
        0,
        TOP_HEIGHT,
        MEDIA_WIDTH,
        WORKSPACE_BOTTOM - TOP_HEIGHT,
        PANEL,
    );
    canvas.rect(
        MEDIA_WIDTH - 1,
        TOP_HEIGHT,
        1,
        WORKSPACE_BOTTOM - TOP_HEIGHT,
        DIVIDER,
    );
    panel_title(canvas, 0, TOP_HEIGHT, MEDIA_WIDTH, "MEDIA", Icon::Folder);

    canvas.rect(12, 82, 175, 28, FIELD);
    canvas.outline(12, 82, 175, 28, 1, DIVIDER_SOFT);
    icons::draw(canvas, Icon::Search, 21, 89, 14, TEXT_MUTED);
    canvas.text_sized(44, 90, "Search media", 10.5, TEXT_MUTED);
    canvas.rect(195, 82, 70, 28, SURFACE);
    icons::draw(canvas, Icon::Upload, 204, 89, 14, TEXT_SECONDARY);
    canvas.text_sized(225, 90, "Add", 10.5, TEXT_SECONDARY);

    let video_count = project
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Video)
        .count();
    let audio_count = project.assets.len().saturating_sub(video_count);
    media_filter(
        canvas,
        12,
        121,
        78,
        Icon::Folder,
        "All",
        project.assets.len(),
        true,
    );
    media_filter(canvas, 96, 121, 78, Icon::Film, "Video", video_count, false);
    media_filter(
        canvas,
        180,
        121,
        85,
        Icon::Music,
        "Audio",
        audio_count,
        false,
    );

    canvas.text_sized(14, 165, "PROJECT FILES", 9.0, TEXT_MUTED);
    canvas.rect(12, 181, MEDIA_WIDTH - 25, 1, DIVIDER_SOFT);

    if project.assets.is_empty() {
        icons::draw(canvas, Icon::Upload, 122, 225, 28, TEXT_MUTED);
        canvas.text_sized(76, 269, "Drop media into this folder", 11.0, TEXT_SECONDARY);
        canvas.text_sized(89, 290, "or use the Add button", 10.0, TEXT_MUTED);
        return;
    }

    let selected_asset_id = project
        .selected_clip_id
        .as_ref()
        .and_then(|id| project.clips.iter().find(|clip| &clip.id == id))
        .map(|clip| clip.asset_id.as_str());
    for (index, asset) in project.assets.iter().take(3).enumerate() {
        let selected = selected_asset_id == Some(asset.id.as_str());
        draw_asset_row(
            canvas,
            project,
            asset,
            190 + index as i32 * 78,
            selected,
            if selected { preview } else { None },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn media_filter(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    icon: Icon,
    label: &str,
    count: usize,
    active: bool,
) {
    canvas.rect(x, y, width, 30, if active { SURFACE } else { PANEL });
    if active {
        canvas.rect(x, y + 29, width, 1, ACCENT);
    }
    icons::draw(
        canvas,
        icon,
        x + 8,
        y + 8,
        14,
        if active { TEXT } else { TEXT_MUTED },
    );
    canvas.text_sized(
        x + 29,
        y + 9,
        label,
        10.0,
        if active { TEXT_SECONDARY } else { TEXT_MUTED },
    );
    canvas.text_sized(x + width - 16, y + 9, &count.to_string(), 9.0, TEXT_MUTED);
}

fn draw_asset_row(
    canvas: &mut Canvas,
    project: &Project,
    asset: &Asset,
    y: i32,
    selected: bool,
    preview: Option<&RgbaImage>,
) {
    if selected {
        canvas.rect(0, y - 2, MEDIA_WIDTH - 1, 74, PANEL_RAISED);
        canvas.rect(0, y - 2, 3, 74, ACCENT);
    }
    canvas.rect(14, y + 5, 90, 51, [7, 8, 10, 255]);
    if let Some(preview) = preview {
        canvas.blit_fit(preview, 14, y + 5, 90, 51);
    } else {
        canvas.rect(15, y + 6, 88, 49, [27, 31, 34, 255]);
        icons::draw(
            canvas,
            if asset.kind == AssetKind::Video {
                Icon::Film
            } else {
                Icon::Music
            },
            48,
            y + 18,
            24,
            TEXT_MUTED,
        );
    }
    canvas.outline(14, y + 5, 90, 51, 1, DIVIDER);

    canvas.text_sized(116, y + 7, &truncate(&asset.name, 20), 10.8, TEXT);
    canvas.text_sized(
        116,
        y + 29,
        &format!("{} × {}", asset.width, asset.height),
        9.5,
        TEXT_MUTED,
    );
    canvas.text_sized(
        226,
        y + 29,
        &short_time(asset.duration_frames, project),
        9.5,
        TEXT_MUTED,
    );
    canvas.rect(14, y + 69, MEDIA_WIDTH - 29, 1, DIVIDER_SOFT);
}

fn draw_program_monitor(
    canvas: &mut Canvas,
    project: &Project,
    preview: Option<&RgbaImage>,
    playing: bool,
) {
    let width = INSPECTOR_X - MEDIA_WIDTH;
    canvas.rect(
        MEDIA_WIDTH,
        TOP_HEIGHT,
        width,
        WORKSPACE_BOTTOM - TOP_HEIGHT,
        BACKGROUND,
    );
    panel_title(
        canvas,
        MEDIA_WIDTH,
        TOP_HEIGHT,
        width,
        "PROGRAM",
        Icon::Film,
    );
    canvas.text_sized(904, 57, "FIT", 9.0, TEXT_MUTED);

    let monitor_x = 294;
    let monitor_y = 88;
    let monitor_width = 646;
    let monitor_height = 290;
    canvas.rect(
        monitor_x,
        monitor_y,
        monitor_width,
        monitor_height,
        [3, 4, 5, 255],
    );
    if let Some(preview) = preview {
        canvas.blit_fit(
            preview,
            monitor_x,
            monitor_y,
            monitor_width as u32,
            monitor_height as u32,
        );
    } else {
        icons::draw(canvas, Icon::Film, 595, 205, 44, TEXT_MUTED);
    }
    canvas.outline(
        monitor_x,
        monitor_y,
        monitor_width,
        monitor_height,
        1,
        DIVIDER,
    );

    canvas.text_sized(
        304,
        397,
        &format_timecode(project.playhead_frame, project.fps),
        11.5,
        ACCENT,
    );
    draw_transport(canvas, 617, 398, playing);
    canvas.text_sized(
        889,
        397,
        &short_time(project.duration_frames(), project),
        10.5,
        TEXT_MUTED,
    );
}

fn draw_transport(canvas: &mut Canvas, center_x: i32, y: i32, playing: bool) {
    transport_button(canvas, center_x - 92, y, Icon::SkipBack, false);
    transport_button(canvas, center_x - 50, y, Icon::StepBack, false);
    transport_button(
        canvas,
        center_x,
        y - 3,
        if playing { Icon::Pause } else { Icon::Play },
        true,
    );
    transport_button(canvas, center_x + 50, y, Icon::StepForward, false);
    transport_button(canvas, center_x + 92, y, Icon::SkipForward, false);
}

fn transport_button(canvas: &mut Canvas, x: i32, y: i32, icon: Icon, primary: bool) {
    let size = if primary { 34 } else { 28 };
    if primary {
        canvas.rect(x - size / 2, y - 6, size, size, SURFACE);
    }
    icons::draw(
        canvas,
        icon,
        x - if primary { 9 } else { 8 },
        y + if primary { 2 } else { 3 },
        if primary { 18 } else { 16 },
        if primary { TEXT } else { TEXT_SECONDARY },
    );
}

fn draw_inspector(canvas: &mut Canvas, project: &Project) {
    canvas.rect(
        INSPECTOR_X,
        TOP_HEIGHT,
        WIDTH - INSPECTOR_X,
        WORKSPACE_BOTTOM - TOP_HEIGHT,
        PANEL,
    );
    canvas.rect(
        INSPECTOR_X,
        TOP_HEIGHT,
        1,
        WORKSPACE_BOTTOM - TOP_HEIGHT,
        DIVIDER,
    );
    panel_title(
        canvas,
        INSPECTOR_X,
        TOP_HEIGHT,
        WIDTH - INSPECTOR_X,
        "INSPECTOR",
        Icon::Sliders,
    );

    inspector_tab(canvas, 969, 82, 56, "Video", true);
    inspector_tab(canvas, 1028, 82, 56, "Audio", false);
    inspector_tab(canvas, 1087, 82, 70, "Effects", false);
    canvas.rect(INSPECTOR_X, 113, WIDTH - INSPECTOR_X, 1, DIVIDER_SOFT);

    let selected = project
        .selected_clip_id
        .as_ref()
        .and_then(|id| project.clips.iter().find(|clip| &clip.id == id));
    let Some(clip) = selected else {
        icons::draw(canvas, Icon::MousePointer, 1063, 173, 28, TEXT_MUTED);
        canvas.text_sized(1023, 219, "Select a timeline clip", 10.5, TEXT_MUTED);
        return;
    };
    let asset = project.asset(&clip.asset_id);

    canvas.text_sized(
        970,
        127,
        &truncate(
            asset
                .map(|item| item.name.as_str())
                .unwrap_or("Missing media"),
            27,
        ),
        10.5,
        TEXT_SECONDARY,
    );
    canvas.text_sized(
        1157,
        127,
        &short_time(clip.duration_frames, project),
        9.5,
        TEXT_MUTED,
    );
    canvas.rect(INSPECTOR_X, 151, WIDTH - INSPECTOR_X, 1, DIVIDER_SOFT);

    inspector_section(canvas, 164, "TRANSFORM");
    inspector_field(canvas, 192, "Position", "0.0     0.0");
    inspector_slider(canvas, 224, "Scale", "100.0%");
    inspector_field(canvas, 256, "Rotation", "0.0°");

    canvas.rect(INSPECTOR_X, 296, WIDTH - INSPECTOR_X, 1, DIVIDER_SOFT);
    inspector_section(canvas, 309, "TIMING");
    inspector_field(
        canvas,
        337,
        "Start",
        &format_timecode(clip.start_frame, project.fps),
    );
    inspector_field(
        canvas,
        369,
        "Duration",
        &format_timecode(clip.duration_frames, project.fps),
    );
    if let Some(asset) = asset {
        canvas.text_sized(971, 413, "SOURCE", 9.0, TEXT_MUTED);
        canvas.text_sized(
            1129,
            413,
            &format!("{}×{}", asset.width, asset.height),
            9.5,
            TEXT_SECONDARY,
        );
    }
}

fn inspector_tab(canvas: &mut Canvas, x: i32, y: i32, width: i32, label: &str, active: bool) {
    canvas.text_sized(
        x + 5,
        y + 8,
        label,
        10.0,
        if active { TEXT } else { TEXT_MUTED },
    );
    if active {
        canvas.rect(x, y + 29, width, 2, ACCENT);
    }
}

fn inspector_section(canvas: &mut Canvas, y: i32, label: &str) {
    icons::draw(canvas, Icon::ChevronDown, 970, y + 1, 13, TEXT_MUTED);
    canvas.text_sized(991, y + 2, label, 9.5, TEXT_SECONDARY);
}

fn inspector_field(canvas: &mut Canvas, y: i32, label: &str, value: &str) {
    canvas.text_sized(970, y + 8, label, 10.0, TEXT_MUTED);
    canvas.rect(1065, y, 121, 27, FIELD);
    canvas.outline(1065, y, 121, 27, 1, DIVIDER_SOFT);
    canvas.text_sized(1076, y + 8, &truncate(value, 17), 10.0, TEXT_SECONDARY);
}

fn inspector_slider(canvas: &mut Canvas, y: i32, label: &str, value: &str) {
    canvas.text_sized(970, y + 8, label, 10.0, TEXT_MUTED);
    canvas.rect(1034, y + 13, 70, 2, DIVIDER);
    canvas.rect(1069, y + 10, 3, 8, ACCENT);
    canvas.rect(1114, y, 72, 27, FIELD);
    canvas.outline(1114, y, 72, 27, 1, DIVIDER_SOFT);
    canvas.text_sized(1124, y + 8, value, 10.0, TEXT_SECONDARY);
}

fn draw_timeline(canvas: &mut Canvas, project: &Project) {
    canvas.rect(0, WORKSPACE_BOTTOM, WIDTH, 286, BACKGROUND);
    canvas.rect(0, WORKSPACE_BOTTOM, WIDTH, 1, DIVIDER);
    canvas.rect(0, WORKSPACE_BOTTOM, WIDTH, 38, CHROME);
    canvas.text_sized(14, 456, "TIMELINE", 10.5, TEXT_SECONDARY);

    tool_button(canvas, 96, 449, 76, Icon::MousePointer, "Select", true);
    tool_button(canvas, 178, 449, 72, Icon::Scissors, "Blade", false);
    tool_button(canvas, 256, 449, 68, Icon::Magnet, "Snap", false);
    canvas.text_sized(1034, 456, "ZOOM", 9.0, TEXT_MUTED);
    canvas.rect(1080, 462, 102, 2, DIVIDER);
    canvas.rect(1127, 458, 4, 10, ACCENT);

    canvas.rect(0, 480, WIDTH, 28, [15, 16, 18, 255]);
    canvas.rect(0, TIMELINE_Y, TIMELINE_X, TRACK_HEIGHT * 2, PANEL);
    canvas.rect(
        TIMELINE_X,
        TIMELINE_Y,
        TIMELINE_WIDTH,
        TRACK_HEIGHT * 2,
        [11, 12, 14, 255],
    );
    canvas.rect(TIMELINE_X - 1, 480, 1, TRACK_HEIGHT * 2 + 28, DIVIDER);
    canvas.rect(0, TIMELINE_Y + TRACK_HEIGHT, WIDTH, 1, DIVIDER);

    draw_time_ruler(canvas, project);
    draw_track_header(canvas, TIMELINE_Y, "V1", "VIDEO", true);
    draw_track_header(canvas, TIMELINE_Y + TRACK_HEIGHT, "A1", "AUDIO", false);

    let duration = project.duration_frames().max(1);
    for clip in &project.clips {
        let x = TIMELINE_X
            + ((clip.start_frame as f64 / duration as f64) * TIMELINE_WIDTH as f64) as i32;
        let width = (((clip.duration_frames as f64 / duration as f64) * TIMELINE_WIDTH as f64)
            as i32)
            .max(8);
        let selected = project.selected_clip_id.as_deref() == Some(&clip.id);
        let asset = project.asset(&clip.asset_id);
        let is_audio = clip.track_id.starts_with('A');
        let y = if is_audio {
            TIMELINE_Y + TRACK_HEIGHT + 5
        } else {
            TIMELINE_Y + 5
        };
        draw_clip(
            canvas,
            x,
            y,
            width,
            TRACK_HEIGHT - 10,
            clip,
            asset,
            project,
            selected,
            is_audio,
        );
        if !is_audio && asset.is_some_and(|item| item.has_audio) {
            draw_linked_audio(
                canvas,
                x,
                TIMELINE_Y + TRACK_HEIGHT + 5,
                width,
                TRACK_HEIGHT - 10,
                selected,
            );
        }
    }

    let playhead_x = TIMELINE_X
        + ((project.playhead_frame as f64 / duration as f64) * TIMELINE_WIDTH as f64) as i32;
    canvas.rect(playhead_x, 480, 2, TRACK_HEIGHT * 2 + 28, PLAYHEAD);
    canvas.triangle(
        (playhead_x - 5, 482),
        (playhead_x + 7, 482),
        (playhead_x + 1, 489),
        PLAYHEAD,
    );

    canvas.rect(0, 664, WIDTH, 28, [15, 16, 18, 255]);
    canvas.rect(143, 677, 1025, 2, DIVIDER);
    canvas.rect(143, 674, 430, 8, [78, 84, 92, 255]);
}

fn tool_button(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    icon: Icon,
    label: &str,
    active: bool,
) {
    canvas.rect(x, y, width, 25, if active { SURFACE } else { CHROME });
    if active {
        canvas.rect(x, y + 24, width, 1, ACCENT);
    }
    icons::draw(
        canvas,
        icon,
        x + 8,
        y + 6,
        13,
        if active { TEXT } else { TEXT_MUTED },
    );
    canvas.text_sized(
        x + 28,
        y + 7,
        label,
        9.5,
        if active { TEXT_SECONDARY } else { TEXT_MUTED },
    );
}

fn draw_time_ruler(canvas: &mut Canvas, project: &Project) {
    let duration = project.duration_frames().max(1);
    for tick in 0..=10 {
        let x = TIMELINE_X + tick * TIMELINE_WIDTH / 10;
        canvas.rect(x, 497, 1, if tick % 2 == 0 { 11 } else { 6 }, DIVIDER);
        if tick % 2 == 0 {
            let frame = duration * tick as i64 / 10;
            let label = short_time_with_seconds(frame, project);
            canvas.text_sized(
                if tick == 10 { x - 47 } else { x + 5 },
                484,
                &label,
                8.8,
                TEXT_MUTED,
            );
        }
    }
}

fn draw_track_header(canvas: &mut Canvas, y: i32, id: &str, kind: &str, video: bool) {
    canvas.text_sized(14, y + 14, id, 11.5, if video { ACCENT } else { AUDIO });
    canvas.text_sized(14, y + 38, kind, 8.8, TEXT_MUTED);
    track_icon_button(canvas, 55, y + 12, Icon::Lock);
    track_icon_button(
        canvas,
        79,
        y + 12,
        if video { Icon::Eye } else { Icon::Volume },
    );
    track_icon_button(
        canvas,
        103,
        y + 12,
        if video {
            Icon::VolumeMuted
        } else {
            Icon::Volume
        },
    );
}

fn track_icon_button(canvas: &mut Canvas, x: i32, y: i32, icon: Icon) {
    canvas.rect(x, y, 21, 21, FIELD);
    icons::draw(canvas, icon, x + 4, y + 4, 13, TEXT_MUTED);
}

#[allow(clippy::too_many_arguments)]
fn draw_clip(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    clip: &Clip,
    asset: Option<&Asset>,
    project: &Project,
    selected: bool,
    audio: bool,
) {
    let border = if selected {
        ACCENT
    } else if audio {
        AUDIO
    } else {
        [55, 111, 102, 255]
    };
    canvas.rect(x, y, width, height, border);
    canvas.rect(
        x + 1,
        y + 1,
        width - 2,
        height - 2,
        if audio { AUDIO_FILL } else { VIDEO_FILL },
    );
    if selected {
        canvas.outline(x, y, width, height, 2, ACCENT);
    }

    if audio {
        draw_waveform(canvas, x + 4, y + 7, width - 8, height - 14, AUDIO);
        return;
    }

    canvas.rect(x + 1, y + 1, width - 2, 18, VIDEO_TOP);
    let frame_width = 42;
    for frame_x in (x + 3..x + width - 2).step_by(frame_width as usize) {
        let visible_width = frame_width.min(x + width - 2 - frame_x).max(0);
        canvas.rect(frame_x, y + 21, visible_width - 1, 34, [34, 68, 64, 255]);
        canvas.rect(frame_x, y + 39, visible_width - 1, 16, [25, 51, 49, 255]);
    }
    let name = asset
        .map(|item| item.name.as_str())
        .unwrap_or("Missing media");
    canvas.text_sized(
        x + 6,
        y + 5,
        &truncate(name, ((width - 10) / 7).max(1) as usize),
        9.2,
        TEXT,
    );
    if width > 80 {
        canvas.text_sized(
            x + 6,
            y + 57,
            &short_time(clip.duration_frames, project),
            8.8,
            [150, 184, 176, 255],
        );
    }
}

fn draw_linked_audio(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, selected: bool) {
    canvas.rect(x, y, width, height, if selected { ACCENT } else { AUDIO });
    canvas.rect(x + 1, y + 1, width - 2, height - 2, AUDIO_FILL);
    if selected {
        canvas.outline(x, y, width, height, 2, ACCENT);
    }
    draw_waveform(canvas, x + 4, y + 7, width - 8, height - 14, AUDIO);
}

fn draw_waveform(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, color: Color) {
    if width <= 0 || height <= 0 {
        return;
    }
    let middle = y + height / 2;
    for offset in (0..width).step_by(3) {
        let phase = ((offset * 37 + width * 11) % 101) as f32 / 100.0;
        let amplitude = (phase * height as f32 * 0.43).max(1.0) as i32;
        canvas.rect(x + offset, middle - amplitude, 1, amplitude * 2, color);
    }
    canvas.rect(x, middle, width, 1, [112, 83, 43, 255]);
}

fn draw_status_bar(canvas: &mut Canvas, status: &str) {
    canvas.rect(0, 728, WIDTH, HEIGHT - 728, CHROME);
    canvas.rect(0, 728, WIDTH, 1, DIVIDER);
    canvas.text_sized(14, 739, &truncate(status, 96), 9.5, TEXT_SECONDARY);
    canvas.text_sized(
        903,
        739,
        "←/→  SCRUB     S  SPLIT     DEL  REMOVE     Q  QUIT",
        8.8,
        TEXT_MUTED,
    );
}

fn panel_title(canvas: &mut Canvas, x: i32, y: i32, width: i32, title: &str, icon: Icon) {
    canvas.rect(x, y, width, 32, PANEL_RAISED);
    canvas.rect(x, y + 31, width, 1, DIVIDER_SOFT);
    icons::draw(canvas, icon, x + 12, y + 9, 14, TEXT_MUTED);
    canvas.text_sized(x + 36, y + 10, title, 9.5, TEXT_SECONDARY);
}

fn short_time(frames: i64, project: &Project) -> String {
    let seconds = project.fps.frames_to_seconds(frames.max(0)).round() as i64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn short_time_with_seconds(frames: i64, project: &Project) -> String {
    let seconds = project.fps.frames_to_seconds(frames.max(0)).round() as i64;
    format!("{:02}:{:02}.00", seconds / 60, seconds % 60)
}

fn truncate(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.to_string();
    }
    value
        .chars()
        .take(maximum.saturating_sub(3))
        .collect::<String>()
        + "..."
}
