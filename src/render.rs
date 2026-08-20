use font8x8::{BASIC_FONTS, UnicodeFonts};
use image::RgbaImage;
#[cfg(test)]
use image::{DynamicImage, ImageFormat};
use pixel_core::fontdue::{Font, FontSettings};
use std::collections::HashMap;
use std::fs;
#[cfg(test)]
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock};

pub type Color = [u8; 4];

/// A logical-coordinate drawing surface backed by pixel-core's raw RGBA canvas.
///
/// The editor UI is still expressed in its 1200x760 design coordinate system,
/// while all rasterization happens directly at the terminal's physical pixel
/// size. Text, paths, and strokes are therefore rasterized at display density
/// instead of being drawn into a small bitmap and enlarged.
pub struct Canvas {
    inner: pixel_core::Canvas,
    scale_x: f32,
    scale_y: f32,
}

impl Canvas {
    pub fn new_viewport(
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        color: Color,
    ) -> Self {
        let physical_width = physical_width.max(1);
        let physical_height = physical_height.max(1);
        let mut inner = pixel_core::Canvas::new(physical_width, physical_height);
        inner.fill(color);
        Self {
            inner,
            scale_x: physical_width as f32 / logical_width.max(1) as f32,
            scale_y: physical_height as f32 / logical_height.max(1) as f32,
        }
    }

    pub fn core(&self) -> &pixel_core::Canvas {
        &self.inner
    }

    #[cfg(test)]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.inner.width, self.inner.height)
    }

    pub fn rgba_image(&self) -> RgbaImage {
        RgbaImage::from_raw(
            self.inner.width,
            self.inner.height,
            self.inner.pixels.clone(),
        )
        .expect("pixel-core canvas always contains width * height * 4 bytes")
    }

    pub fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        let (x, y, width, height) = self.physical_box(x, y, width, height);
        self.inner
            .fill_rounded_rect(x, y, width, height, [0.0; 4], color);
    }

    pub fn outline(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        thickness: i32,
        color: Color,
    ) {
        let (x, y, width, height) = self.physical_box(x, y, width, height);
        self.inner.stroke_rounded_rect(
            x,
            y,
            width,
            height,
            [0.0; 4],
            thickness.max(1) as f32 * self.uniform_scale(),
            color,
        );
    }

    pub fn triangle(
        &mut self,
        first: (i32, i32),
        second: (i32, i32),
        third: (i32, i32),
        color: Color,
    ) {
        let mut builder = tiny_skia::PathBuilder::new();
        builder.move_to(first.0 as f32 * self.scale_x, first.1 as f32 * self.scale_y);
        builder.line_to(
            second.0 as f32 * self.scale_x,
            second.1 as f32 * self.scale_y,
        );
        builder.line_to(third.0 as f32 * self.scale_x, third.1 as f32 * self.scale_y);
        builder.close();
        if let Some(path) = builder.finish() {
            self.inner.fill_path(&path, color);
        }
    }

    /// Rasterizes an authored SVG at the terminal's physical density and
    /// caches the result. This lets the UI consume real icon assets without
    /// reducing them to a home-grown path subset.
    pub fn svg_icon(&mut self, x: i32, y: i32, size: i32, svg: &'static str, color: Color) {
        let logical_size = size.max(1) as f32;
        let pixel_size = (logical_size * self.uniform_scale()).round().max(1.0) as u32;
        let Some(pixels) = cached_svg_icon(svg, pixel_size, color) else {
            return;
        };
        let box_width = logical_size * self.scale_x;
        let box_height = logical_size * self.scale_y;
        let physical_x = x as f32 * self.scale_x + (box_width - pixel_size as f32) / 2.0;
        let physical_y = y as f32 * self.scale_y + (box_height - pixel_size as f32) / 2.0;
        self.inner.blit_scaled_rgba(
            physical_x,
            physical_y,
            pixel_size as f32,
            pixel_size as f32,
            &pixels,
            pixel_size,
            pixel_size,
        );
    }

    pub fn text_sized(&mut self, x: i32, y: i32, text: &str, size: f32, color: Color) {
        if let Some(font) = ui_font() {
            let pixels = (size * self.scale_y).max(7.0);
            let ascent = font
                .horizontal_line_metrics(pixels)
                .map_or(pixels * 0.8, |metrics| metrics.ascent);
            self.inner.draw_text(
                font,
                text,
                (x as f32 * self.scale_x).round() as i32,
                (y as f32 * self.scale_y + ascent).round() as i32,
                pixels,
                color,
            );
            return;
        }
        let scale = (size / 10.0).round().max(1.0) as u32;
        self.bitmap_text(x, y, text, scale, color);
    }

    fn bitmap_text(&mut self, x: i32, y: i32, text: &str, scale: u32, color: Color) {
        let mut cursor_x = x;
        let scale = scale.max(1);
        for character in text.chars() {
            if let Some(glyph) = BASIC_FONTS.get(character) {
                for (row, bits) in glyph.iter().enumerate() {
                    for column in 0..8 {
                        if bits & (1 << column) != 0 {
                            self.rect(
                                cursor_x + column * scale as i32,
                                y + row as i32 * scale as i32,
                                scale as i32,
                                scale as i32,
                                color,
                            );
                        }
                    }
                }
            }
            cursor_x += 9 * scale as i32;
        }
    }

    pub fn blit_fit(&mut self, source: &RgbaImage, x: i32, y: i32, width: u32, height: u32) {
        if source.width() == 0 || source.height() == 0 || width == 0 || height == 0 {
            return;
        }
        let box_x = x as f32 * self.scale_x;
        let box_y = y as f32 * self.scale_y;
        let box_width = width as f32 * self.scale_x;
        let box_height = height as f32 * self.scale_y;
        let ratio = (box_width / source.width() as f32).min(box_height / source.height() as f32);
        let target_width = source.width() as f32 * ratio;
        let target_height = source.height() as f32 * ratio;
        self.inner.blit_scaled_rgba(
            box_x + (box_width - target_width) / 2.0,
            box_y + (box_height - target_height) / 2.0,
            target_width,
            target_height,
            source.as_raw(),
            source.width(),
            source.height(),
        );
    }

    #[cfg(test)]
    pub fn png_bytes(&self) -> image::ImageResult<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(self.rgba_image()).write_to(&mut cursor, ImageFormat::Png)?;
        Ok(cursor.into_inner())
    }

    fn physical_box(&self, x: i32, y: i32, width: i32, height: i32) -> (f32, f32, f32, f32) {
        let left = (x as f32 * self.scale_x).round();
        let top = (y as f32 * self.scale_y).round();
        let right = ((x + width) as f32 * self.scale_x).round();
        let bottom = ((y + height) as f32 * self.scale_y).round();
        (left, top, right - left, bottom - top)
    }

    fn uniform_scale(&self) -> f32 {
        self.scale_x.min(self.scale_y)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SvgIconKey {
    svg: &'static str,
    pixel_size: u32,
    color: Color,
}

fn cached_svg_icon(svg: &'static str, pixel_size: u32, color: Color) -> Option<Arc<[u8]>> {
    static CACHE: OnceLock<Mutex<HashMap<SvgIconKey, Arc<[u8]>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = SvgIconKey {
        svg,
        pixel_size,
        color,
    };
    if let Some(pixels) = cache.lock().ok()?.get(&key).cloned() {
        return Some(pixels);
    }

    let tint = format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]);
    let source = svg.replace("currentColor", &tint);
    let tree = resvg::usvg::Tree::from_str(&source, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixel_size, pixel_size)?;
    let size = tree.size();
    let scale = (pixel_size as f32 / size.width()).min(pixel_size as f32 / size.height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let pixels: Arc<[u8]> = Arc::from(pixmap.data().to_vec());
    cache.lock().ok()?.insert(key, pixels.clone());
    Some(pixels)
}

fn ui_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(load_ui_font).as_ref()
}

fn load_ui_font() -> Option<Font> {
    const FONT_PATHS: &[&str] = &[
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFCompact.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/usr/share/fonts/truetype/inter/InterVariable.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ];
    FONT_PATHS.iter().find_map(|path| {
        fs::read(path)
            .ok()
            .and_then(|bytes| Font::from_bytes(bytes, FontSettings::default()).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_encodes_as_png() {
        let mut canvas = Canvas::new_viewport(64, 32, 64, 32, [0, 0, 0, 255]);
        canvas.rect(2, 2, 20, 10, [255, 0, 0, 255]);
        canvas.text_sized(4, 16, "TE", 12.0, [255, 255, 255, 255]);
        let bytes = canvas.png_bytes().unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn canvas_renders_at_physical_viewport_size() {
        let canvas = Canvas::new_viewport(64, 32, 192, 96, [0, 0, 0, 255]);
        assert_eq!(canvas.dimensions(), (192, 96));
    }

    #[test]
    fn svg_icons_render_at_display_density() {
        let mut canvas = Canvas::new_viewport(24, 24, 96, 96, [0, 0, 0, 255]);
        canvas.svg_icon(0, 0, 24, include_str!("../assets/icons/play.svg"), [255; 4]);
        assert!(canvas.core().pixels.iter().any(|value| *value > 0));
    }
}
