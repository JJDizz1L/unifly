//! Theme-aware chart colour interpolation and terminal quantization.

use ratatui::style::Color;

use crate::tui::render_caps::{ColorDepth, RenderCaps};

#[derive(Debug, Clone, Copy)]
pub struct ChartGradient {
    start: Color,
    end: Color,
}

#[derive(Debug, Clone, Copy)]
struct Oklab {
    lightness: f64,
    green_red: f64,
    blue_yellow: f64,
}

impl ChartGradient {
    pub const fn new(start: Color, end: Color) -> Self {
        Self { start, end }
    }

    pub fn bands(self, caps: RenderCaps, count: usize) -> Vec<Color> {
        let count = count.max(1);
        (0..count)
            .map(|idx| {
                #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
                let t = if count == 1 {
                    1.0
                } else {
                    idx as f64 / (count - 1) as f64
                };
                quantize_color(oklab_lerp(self.start, self.end, t), caps.color_depth)
            })
            .collect()
    }
}

pub fn oklab_lerp(start: Color, end: Color, t: f64) -> Color {
    let start = Oklab::from_rgb(color_to_rgb(start));
    let end = Oklab::from_rgb(color_to_rgb(end));
    let t = t.clamp(0.0, 1.0);
    Oklab {
        lightness: start.lightness + (end.lightness - start.lightness) * t,
        green_red: start.green_red + (end.green_red - start.green_red) * t,
        blue_yellow: start.blue_yellow + (end.blue_yellow - start.blue_yellow) * t,
    }
    .to_color()
}

fn quantize_color(color: Color, depth: ColorDepth) -> Color {
    match depth {
        ColorDepth::NoColor => Color::Reset,
        ColorDepth::TrueColor => color,
        ColorDepth::Ansi256 => {
            let (r, g, b) = color_to_rgb(color);
            let r = quantize_cube_channel(r);
            let g = quantize_cube_channel(g);
            let b = quantize_cube_channel(b);
            Color::Indexed(16 + (36 * r) + (6 * g) + b)
        }
        ColorDepth::Ansi16 => nearest_ansi16(color),
    }
}

fn quantize_cube_channel(value: u8) -> u8 {
    let channel = (u16::from(value) * 5 + 127) / 255;
    u8::try_from(channel).unwrap_or(5)
}

fn nearest_ansi16(color: Color) -> Color {
    const PALETTE: &[(Color, (u8, u8, u8))] = &[
        (Color::Black, (0, 0, 0)),
        (Color::Red, (170, 0, 0)),
        (Color::Green, (0, 170, 0)),
        (Color::Yellow, (170, 85, 0)),
        (Color::Blue, (0, 0, 170)),
        (Color::Magenta, (170, 0, 170)),
        (Color::Cyan, (0, 170, 170)),
        (Color::Gray, (170, 170, 170)),
        (Color::DarkGray, (85, 85, 85)),
        (Color::LightRed, (255, 85, 85)),
        (Color::LightGreen, (85, 255, 85)),
        (Color::LightYellow, (255, 255, 85)),
        (Color::LightBlue, (85, 85, 255)),
        (Color::LightMagenta, (255, 85, 255)),
        (Color::LightCyan, (85, 255, 255)),
        (Color::White, (255, 255, 255)),
    ];

    let rgb = color_to_rgb(color);
    PALETTE
        .iter()
        .min_by_key(|(_, candidate)| rgb_distance(rgb, *candidate))
        .map_or(Color::Gray, |(color, _)| *color)
}

fn rgb_distance(left: (u8, u8, u8), right: (u8, u8, u8)) -> u32 {
    let dr = i32::from(left.0) - i32::from(right.0);
    let dg = i32::from(left.1) - i32::from(right.1);
    let db = i32::from(left.2) - i32::from(right.2);
    (dr * dr + dg * dg + db * db).cast_unsigned()
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Reset => (192, 192, 192),
        Color::Black => (0, 0, 0),
        Color::Red => (170, 0, 0),
        Color::Green => (0, 170, 0),
        Color::Yellow => (170, 85, 0),
        Color::Blue => (0, 0, 170),
        Color::Magenta => (170, 0, 170),
        Color::Cyan => (0, 170, 170),
        Color::Gray => (170, 170, 170),
        Color::DarkGray => (85, 85, 85),
        Color::LightRed => (255, 85, 85),
        Color::LightGreen => (85, 255, 85),
        Color::LightYellow => (255, 255, 85),
        Color::LightBlue => (85, 85, 255),
        Color::LightMagenta => (255, 85, 255),
        Color::LightCyan => (85, 255, 255),
        Color::White => (255, 255, 255),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(index) => indexed_to_rgb(index),
    }
}

fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0 => color_to_rgb(Color::Black),
        1 => color_to_rgb(Color::Red),
        2 => color_to_rgb(Color::Green),
        3 => color_to_rgb(Color::Yellow),
        4 => color_to_rgb(Color::Blue),
        5 => color_to_rgb(Color::Magenta),
        6 => color_to_rgb(Color::Cyan),
        7 => color_to_rgb(Color::Gray),
        8 => color_to_rgb(Color::DarkGray),
        9 => color_to_rgb(Color::LightRed),
        10 => color_to_rgb(Color::LightGreen),
        11 => color_to_rgb(Color::LightYellow),
        12 => color_to_rgb(Color::LightBlue),
        13 => color_to_rgb(Color::LightMagenta),
        14 => color_to_rgb(Color::LightCyan),
        15 => color_to_rgb(Color::White),
        16..=231 => {
            let idx = index - 16;
            (
                STEPS[usize::from(idx / 36)],
                STEPS[usize::from((idx % 36) / 6)],
                STEPS[usize::from(idx % 6)],
            )
        }
        232..=255 => {
            let gray = 8 + ((index - 232) * 10);
            (gray, gray, gray)
        }
    }
}

impl Oklab {
    fn from_rgb((red, green, blue): (u8, u8, u8)) -> Self {
        let linear_red = srgb_to_linear(f64::from(red) / 255.0);
        let linear_green = srgb_to_linear(f64::from(green) / 255.0);
        let linear_blue = srgb_to_linear(f64::from(blue) / 255.0);

        let cone_long = 0.412_221_470_8 * linear_red
            + 0.536_332_536_3 * linear_green
            + 0.051_445_992_9 * linear_blue;
        let cone_medium = 0.211_903_498_2 * linear_red
            + 0.680_699_545_1 * linear_green
            + 0.107_396_956_6 * linear_blue;
        let cone_short = 0.088_302_461_9 * linear_red
            + 0.281_718_837_6 * linear_green
            + 0.629_978_700_5 * linear_blue;

        let cone_long_root = cone_long.cbrt();
        let cone_medium_root = cone_medium.cbrt();
        let cone_short_root = cone_short.cbrt();

        Self {
            lightness: 0.210_454_255_3 * cone_long_root + 0.793_617_785 * cone_medium_root
                - 0.004_072_046_8 * cone_short_root,
            green_red: 1.977_998_495_1 * cone_long_root - 2.428_592_205 * cone_medium_root
                + 0.450_593_709_9 * cone_short_root,
            blue_yellow: 0.025_904_037_1 * cone_long_root + 0.782_771_766_2 * cone_medium_root
                - 0.808_675_766 * cone_short_root,
        }
    }

    fn to_color(self) -> Color {
        let cone_long_root =
            self.lightness + 0.396_337_777_4 * self.green_red + 0.215_803_757_3 * self.blue_yellow;
        let cone_medium_root =
            self.lightness - 0.105_561_345_8 * self.green_red - 0.063_854_172_8 * self.blue_yellow;
        let cone_short_root =
            self.lightness - 0.089_484_177_5 * self.green_red - 1.291_485_548 * self.blue_yellow;

        let cone_long = cone_long_root * cone_long_root * cone_long_root;
        let cone_medium = cone_medium_root * cone_medium_root * cone_medium_root;
        let cone_short = cone_short_root * cone_short_root * cone_short_root;

        let linear_red = 4.076_741_662_1 * cone_long - 3.307_711_591_3 * cone_medium
            + 0.230_969_929_2 * cone_short;
        let linear_green = -1.268_438_004_6 * cone_long + 2.609_757_401_1 * cone_medium
            - 0.341_319_396_5 * cone_short;
        let linear_blue = -0.004_196_086_3 * cone_long - 0.703_418_614_7 * cone_medium
            + 1.707_614_701 * cone_short;

        Color::Rgb(
            linear_to_srgb_u8(linear_red),
            linear_to_srgb_u8(linear_green),
            linear_to_srgb_u8(linear_blue),
        )
    }
}

fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    {
        (srgb * 255.0).round() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::render_caps::{ColorDepth, GlyphTier};

    #[test]
    fn oklab_lerp_preserves_endpoints() {
        assert_eq!(
            oklab_lerp(Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255), 0.0),
            Color::Rgb(0, 0, 0)
        );
        assert_eq!(
            oklab_lerp(Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255), 1.0),
            Color::Rgb(255, 255, 255)
        );
    }

    #[test]
    fn gradient_returns_requested_truecolor_bands() {
        let bands = ChartGradient::new(Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255)).bands(
            RenderCaps {
                color_depth: ColorDepth::TrueColor,
                glyph_tier: GlyphTier::Braille,
            },
            4,
        );

        assert_eq!(bands.len(), 4);
        assert_eq!(bands[0], Color::Rgb(0, 0, 0));
        assert_eq!(bands[3], Color::Rgb(255, 255, 255));
    }

    #[test]
    fn no_color_gradient_resets_every_band() {
        let bands = ChartGradient::new(Color::Red, Color::Blue).bands(
            RenderCaps {
                color_depth: ColorDepth::NoColor,
                glyph_tier: GlyphTier::Block,
            },
            3,
        );

        assert_eq!(bands, vec![Color::Reset, Color::Reset, Color::Reset]);
    }

    #[test]
    fn ansi256_gradient_quantizes_to_indexed_colors() {
        let bands = ChartGradient::new(Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255)).bands(
            RenderCaps {
                color_depth: ColorDepth::Ansi256,
                glyph_tier: GlyphTier::Braille,
            },
            2,
        );

        assert!(matches!(
            bands.as_slice(),
            [Color::Indexed(_), Color::Indexed(_)]
        ));
    }

    #[test]
    fn ansi16_gradient_uses_basic_palette() {
        let bands = ChartGradient::new(Color::Rgb(0, 0, 180), Color::Rgb(0, 220, 220)).bands(
            RenderCaps {
                color_depth: ColorDepth::Ansi16,
                glyph_tier: GlyphTier::Braille,
            },
            3,
        );

        assert!(
            bands
                .iter()
                .all(|color| !matches!(color, Color::Rgb(_, _, _) | Color::Indexed(_)))
        );
    }
}
