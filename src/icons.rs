use crate::render::{Canvas, Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    ChevronDown,
    Download,
    Eye,
    Film,
    Folder,
    Lock,
    Magnet,
    MousePointer,
    Music,
    Pause,
    Play,
    Scissors,
    Search,
    Sliders,
    SkipBack,
    SkipForward,
    StepBack,
    StepForward,
    Upload,
    Volume,
    VolumeMuted,
}

impl Icon {
    fn svg(self) -> &'static str {
        match self {
            Self::ChevronDown => include_str!("../assets/icons/chevron-down.svg"),
            Self::Download => include_str!("../assets/icons/download.svg"),
            Self::Eye => include_str!("../assets/icons/eye.svg"),
            Self::Film => include_str!("../assets/icons/film.svg"),
            Self::Folder => include_str!("../assets/icons/folder.svg"),
            Self::Lock => include_str!("../assets/icons/lock.svg"),
            Self::Magnet => include_str!("../assets/icons/magnet.svg"),
            Self::MousePointer => include_str!("../assets/icons/mouse-pointer-2.svg"),
            Self::Music => include_str!("../assets/icons/music-2.svg"),
            Self::Pause => include_str!("../assets/icons/pause.svg"),
            Self::Play => include_str!("../assets/icons/play.svg"),
            Self::Scissors => include_str!("../assets/icons/scissors.svg"),
            Self::Search => include_str!("../assets/icons/search.svg"),
            Self::Sliders => include_str!("../assets/icons/sliders-horizontal.svg"),
            Self::SkipBack => include_str!("../assets/icons/skip-back.svg"),
            Self::SkipForward => include_str!("../assets/icons/skip-forward.svg"),
            Self::StepBack => include_str!("../assets/icons/step-back.svg"),
            Self::StepForward => include_str!("../assets/icons/step-forward.svg"),
            Self::Upload => include_str!("../assets/icons/upload.svg"),
            Self::Volume => include_str!("../assets/icons/volume-2.svg"),
            Self::VolumeMuted => include_str!("../assets/icons/volume-x.svg"),
        }
    }
}

pub fn draw(canvas: &mut Canvas, icon: Icon, x: i32, y: i32, size: i32, color: Color) {
    canvas.svg_icon(x, y, size, icon.svg(), color);
}
