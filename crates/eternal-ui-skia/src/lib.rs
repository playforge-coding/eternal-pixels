// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Skia rendering for [eternal-ui](eternal_ui).
//!
//! Two types implement the toolkit's backend traits: [`SkiaFonts`] measures
//! text (the [`Fonts`] trait) and [`SkiaRenderer`] draws one frame onto a
//! Skia `Canvas` (the [`Renderer`] trait). Where the canvas comes from is up
//! to the application: a CPU raster surface, or a GPU surface through one
//! of skia-safe's `gl`, `metal` or `vulkan` features.
//!
//! ```
//! use eternal_ui::{ui, Element, Size, Ui};
//! use eternal_ui_skia::{SkiaFonts, SkiaRenderer};
//!
//! let mut ui: Ui<()> = Ui::new();
//! ui.set_root(ui! { <panel><button>"Hello"</button></panel> });
//!
//! let fonts = SkiaFonts::new();
//! let mut surface = skia_safe::surfaces::raster_n32_premul((200, 100)).unwrap();
//! ui.layout(Size::new(200.0, 100.0), &fonts);
//! let mut renderer = SkiaRenderer::new(surface.canvas(), &fonts);
//! ui.paint(&mut renderer);
//! ```
//!
//! Shapes are drawn without anti-aliasing and text without subpixel
//! positioning, so 1px borders and pixel fonts stay crisp. Call
//! [`SkiaFonts::set_smooth`] for the ordinary anti-aliased look.
//!
//! Fonts come from the system by default. Load your own, such as a pixel
//! font shipped with the application, with [`SkiaFonts::load_font_file`]
//! or [`SkiaFonts::load_font`], and name them in the stylesheet's
//! `font-family`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use eternal_ui::styler::{Color, Corners, FontStyle, Sides};
use eternal_ui::{FontSpec, Fonts, Point, Rect, Renderer, TextMetrics};
use skia_safe::font::Edging;
use skia_safe::font_style::{Slant, Weight, Width};
use skia_safe::{
    Canvas, ClipOp, Color4f, Data, Font, FontHinting, FontMgr, Paint, PaintStyle, RRect, Typeface,
    Vector,
};

/// Finds fonts and measures text with Skia.
///
/// Typefaces are looked up by the CSS family names in a style, first match
/// wins: fonts loaded with [`load_font`](Self::load_font) and its
/// relatives are checked first, then the system font manager. The generic
/// families `monospace`, `sans-serif`, `serif`, `system-ui` and
/// `ui-monospace` map to common system fonts; anything unknown falls back
/// to the default typeface.
///
/// ```no_run
/// use eternal_ui_skia::SkiaFonts;
///
/// let mut fonts = SkiaFonts::new();
/// // Registered under the family name inside the file...
/// let family = fonts.load_font_file("assets/PixelOperator.ttf")?;
/// // ...or under a name of your choosing, so the stylesheet can say
/// // `font-family: pixel` without caring which file provides it.
/// fonts.load_font_file_as("pixel", "assets/PixelOperator.ttf")?;
/// # Ok::<(), eternal_ui_skia::FontError>(())
/// ```
pub struct SkiaFonts {
    font_mgr: FontMgr,
    custom: Vec<CustomFont>,
    typefaces: RefCell<HashMap<TypefaceKey, Typeface>>,
    smooth: bool,
}

/// A font loaded from data rather than found on the system.
struct CustomFont {
    /// The name the stylesheet uses, lower-cased for matching.
    family: String,
    weight: i32,
    italic: bool,
    typeface: Typeface,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct TypefaceKey {
    families: Vec<String>,
    weight: i32,
    italic: bool,
}

impl Default for SkiaFonts {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaFonts {
    pub fn new() -> Self {
        Self {
            font_mgr: FontMgr::new(),
            custom: Vec::new(),
            typefaces: RefCell::new(HashMap::new()),
            smooth: false,
        }
    }

    /// Loads a font from the bytes of a TrueType, OpenType or WOFF file and
    /// registers it under the family name the file declares, which is
    /// returned. Several weights and styles of one family can be loaded
    /// one file at a time; the closest match to a style's weight and slant
    /// is used.
    pub fn load_font(&mut self, bytes: &[u8]) -> Result<String, FontError> {
        let typeface = self.typeface_from_bytes(bytes)?;
        let family = typeface.family_name();
        self.register(family.clone(), typeface);
        Ok(family)
    }

    /// Like [`load_font`](Self::load_font), but registers the font under
    /// `family` instead of the name in the file, so a stylesheet can refer
    /// to it as `font-family: pixel` whatever the file says.
    pub fn load_font_as(&mut self, family: &str, bytes: &[u8]) -> Result<(), FontError> {
        let typeface = self.typeface_from_bytes(bytes)?;
        self.register(family.to_owned(), typeface);
        Ok(())
    }

    /// [`load_font`](Self::load_font) for a file on disk.
    pub fn load_font_file(&mut self, path: impl AsRef<Path>) -> Result<String, FontError> {
        let bytes = read_font_file(path.as_ref())?;
        self.load_font(&bytes)
            .map_err(|error| error.at(path.as_ref()))
    }

    /// [`load_font_as`](Self::load_font_as) for a file on disk.
    pub fn load_font_file_as(
        &mut self,
        family: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), FontError> {
        let bytes = read_font_file(path.as_ref())?;
        self.load_font_as(family, &bytes)
            .map_err(|error| error.at(path.as_ref()))
    }

    /// The family names of the fonts loaded so far, in load order, as they
    /// were registered.
    pub fn loaded_families(&self) -> impl Iterator<Item = &str> {
        self.custom.iter().map(|font| font.family.as_str())
    }

    fn typeface_from_bytes(&self, bytes: &[u8]) -> Result<Typeface, FontError> {
        self.font_mgr
            .new_from_data(Data::new_copy(bytes), None)
            .ok_or(FontError::Unreadable { path: None })
    }

    fn register(&mut self, family: String, typeface: Typeface) {
        let style = typeface.font_style();
        self.custom.push(CustomFont {
            family: family.to_ascii_lowercase(),
            weight: *style.weight(),
            italic: style.slant() != Slant::Upright,
            typeface,
        });
        // A family that fell back to a system font before may resolve to
        // the new font now.
        self.typefaces.borrow_mut().clear();
    }

    /// The loaded font closest in weight and slant to what was asked for,
    /// among those registered under `family`.
    fn custom_typeface(&self, family: &str, weight: i32, italic: bool) -> Option<Typeface> {
        self.custom
            .iter()
            .filter(|font| font.family.eq_ignore_ascii_case(family))
            .min_by_key(|font| {
                (font.weight - weight).abs() + if font.italic == italic { 0 } else { 1000 }
            })
            .map(|font| font.typeface.clone())
    }

    /// Whether to anti-alias text and shapes. Off by default for a pixel
    /// look.
    pub fn set_smooth(&mut self, smooth: bool) {
        self.smooth = smooth;
    }

    pub fn smooth(&self) -> bool {
        self.smooth
    }

    /// The Skia font for a style's font spec, ready to draw with.
    pub fn font(&self, spec: &FontSpec) -> Font {
        let mut font = Font::new(self.typeface(spec), spec.size);
        if self.smooth {
            font.set_edging(Edging::AntiAlias);
            font.set_subpixel(true);
        } else {
            font.set_edging(Edging::Alias);
            font.set_subpixel(false);
            font.set_hinting(FontHinting::Full);
        }
        font
    }

    fn typeface(&self, spec: &FontSpec) -> Typeface {
        let key = TypefaceKey {
            families: spec.family.names().to_vec(),
            weight: spec.weight.value().round() as i32,
            italic: spec.style != FontStyle::Normal,
        };
        if let Some(typeface) = self.typefaces.borrow().get(&key) {
            return typeface.clone();
        }
        let slant = match spec.style {
            FontStyle::Normal => Slant::Upright,
            FontStyle::Italic => Slant::Italic,
            FontStyle::Oblique => Slant::Oblique,
        };
        let style = skia_safe::FontStyle::new(Weight::from(key.weight), Width::NORMAL, slant);
        let typeface = key
            .families
            .iter()
            .find_map(|family| {
                self.custom_typeface(family, key.weight, key.italic)
                    .or_else(|| {
                        candidates(family)
                            .into_iter()
                            .find_map(|name| self.font_mgr.match_family_style(name, style))
                    })
            })
            .or_else(|| self.font_mgr.legacy_make_typeface(None, style))
            .expect("Skia has a default typeface");
        self.typefaces.borrow_mut().insert(key, typeface.clone());
        typeface
    }
}

fn read_font_file(path: &Path) -> Result<Vec<u8>, FontError> {
    std::fs::read(path).map_err(|source| FontError::Io {
        path: path.display().to_string(),
        source,
    })
}

/// A font could not be loaded.
#[derive(Debug)]
pub enum FontError {
    /// The file could not be read.
    Io {
        path: String,
        source: std::io::Error,
    },
    /// Skia did not recognise the data as a font.
    Unreadable {
        /// The file it came from, when it came from one.
        path: Option<String>,
    },
}

impl FontError {
    fn at(self, path: &Path) -> Self {
        match self {
            Self::Unreadable { path: None } => Self::Unreadable {
                path: Some(path.display().to_string()),
            },
            other => other,
        }
    }
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{path}: {source}"),
            Self::Unreadable { path: Some(path) } => write!(f, "{path}: not a font Skia can read"),
            Self::Unreadable { path: None } => f.write_str("not a font Skia can read"),
        }
    }
}

impl std::error::Error for FontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Unreadable { .. } => None,
        }
    }
}

/// System font names to try for a CSS family name, generic ones expanded.
fn candidates(family: &str) -> Vec<&str> {
    match family {
        "monospace" | "ui-monospace" => vec![
            "Menlo",
            "SF Mono",
            "Consolas",
            "Cascadia Mono",
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Courier New",
            "monospace",
        ],
        "sans-serif" | "system-ui" | "ui-sans-serif" => vec![
            "Helvetica Neue",
            "Segoe UI",
            "DejaVu Sans",
            "Liberation Sans",
            "Arial",
            "sans-serif",
        ],
        "serif" | "ui-serif" => vec![
            "Georgia",
            "Times New Roman",
            "DejaVu Serif",
            "Liberation Serif",
            "serif",
        ],
        other => vec![other],
    }
}

impl Fonts for SkiaFonts {
    fn measure(&self, text: &str, spec: &FontSpec) -> TextMetrics {
        let font = self.font(spec);
        let (_, metrics) = font.metrics();
        let ascent = -metrics.ascent;
        let descent = metrics.descent;
        let natural = (ascent + descent + metrics.leading).ceil();
        TextMetrics {
            width: font.measure_str(text, None).0,
            ascent,
            descent,
            line_height: spec.line_height.unwrap_or(natural),
        }
    }
}

/// Draws one frame of a [`Ui`](eternal_ui::Ui) onto a Skia canvas.
pub struct SkiaRenderer<'a> {
    canvas: &'a Canvas,
    fonts: &'a SkiaFonts,
}

impl<'a> SkiaRenderer<'a> {
    pub fn new(canvas: &'a Canvas, fonts: &'a SkiaFonts) -> Self {
        Self { canvas, fonts }
    }

    fn paint(&self, color: Color) -> Paint {
        let mut paint = Paint::new(Color4f::new(color.r, color.g, color.b, color.a), None);
        paint.set_anti_alias(self.fonts.smooth);
        paint
    }
}

fn skia_rect(rect: Rect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

fn rounded(rect: Rect, radius: Corners<f32>) -> Option<RRect> {
    let radii = [
        radius.top_left,
        radius.top_right,
        radius.bottom_right,
        radius.bottom_left,
    ];
    if radii.iter().all(|&r| r <= 0.0) {
        return None;
    }
    Some(RRect::new_rect_radii(
        skia_rect(rect),
        &radii.map(|r| Vector::new(r, r)),
    ))
}

impl Renderer for SkiaRenderer<'_> {
    fn fill_rect(&mut self, rect: Rect, radius: Corners<f32>, color: Color) {
        let paint = self.paint(color);
        match rounded(rect, radius) {
            Some(rrect) => self.canvas.draw_rrect(rrect, &paint),
            None => self.canvas.draw_rect(skia_rect(rect), &paint),
        };
    }

    fn border(
        &mut self,
        rect: Rect,
        widths: Sides<f32>,
        colors: Sides<Color>,
        radius: Corners<f32>,
    ) {
        if let Some(outer) = rounded(rect, radius) {
            // Rounded borders are stroked with one width and colour; the
            // theme is expected to keep the sides alike when it rounds.
            let width = widths
                .top
                .max(widths.right)
                .max(widths.bottom)
                .max(widths.left);
            let mut paint = self.paint(colors.top);
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            let inset = width / 2.0;
            let mut inner = outer;
            inner.inset((inset, inset));
            self.canvas.draw_rrect(inner, &paint);
            return;
        }
        let strips = [
            (
                Rect::new(rect.x, rect.y, rect.width, widths.top),
                colors.top,
            ),
            (
                Rect::new(
                    rect.x,
                    rect.bottom() - widths.bottom,
                    rect.width,
                    widths.bottom,
                ),
                colors.bottom,
            ),
            (
                Rect::new(
                    rect.x,
                    rect.y + widths.top,
                    widths.left,
                    rect.height - widths.top - widths.bottom,
                ),
                colors.left,
            ),
            (
                Rect::new(
                    rect.right() - widths.right,
                    rect.y + widths.top,
                    widths.right,
                    rect.height - widths.top - widths.bottom,
                ),
                colors.right,
            ),
        ];
        for (strip, color) in strips {
            if strip.width > 0.0 && strip.height > 0.0 && color.a > 0.0 {
                self.canvas.draw_rect(skia_rect(strip), &self.paint(color));
            }
        }
    }

    fn text(&mut self, text: &str, spec: &FontSpec, top_left: Point, color: Color) {
        let font = self.fonts.font(spec);
        let metrics = self.fonts.measure("", spec);
        // Centre the glyph box inside the line box, then drop to the baseline.
        let baseline = top_left.y
            + ((metrics.line_height - (metrics.ascent + metrics.descent)) / 2.0).max(0.0)
            + metrics.ascent;
        let mut paint = self.paint(color);
        paint.set_anti_alias(true);
        self.canvas.draw_str(
            text,
            skia_safe::Point::new(top_left.x, baseline.round()),
            &font,
            &paint,
        );
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.save();
        self.canvas
            .clip_rect(skia_rect(rect), ClipOp::Intersect, false);
    }

    fn pop_clip(&mut self) {
        self.canvas.restore();
    }

    fn push_opacity(&mut self, opacity: f32) {
        self.canvas.save_layer_alpha_f(None, opacity);
    }

    fn pop_opacity(&mut self) {
        self.canvas.restore();
    }
}
