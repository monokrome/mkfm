//! Color types and parsing

use mkframe::{Color, TextColor};

/// RGB color for theme
#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_text_color(self) -> TextColor {
        TextColor::rgb(self.r, self.g, self.b)
    }
}

/// RGBA color for theme
#[derive(Clone, Copy, Debug)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_color(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

/// Parse color strings like "#1e1e23" or "30,30,35"
pub fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if s.starts_with('#') {
        parse_hex_rgb(s)
    } else if s.contains(',') {
        parse_csv_rgb(s)
    } else {
        None
    }
}

fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

fn parse_csv_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parts[0].trim().parse().ok()?;
    let g = parts[1].trim().parse().ok()?;
    let b = parts[2].trim().parse().ok()?;
    Some((r, g, b))
}

/// Parse RGBA color strings like "#1e1e23ff" or "30,30,35,255"
pub fn parse_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    let s = s.trim();
    if s.starts_with('#') {
        parse_hex_rgba(s)
    } else if s.contains(',') {
        parse_csv_rgba(s)
    } else {
        None
    }
}

fn parse_hex_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    let hex = s.trim_start_matches('#');
    if hex.len() == 6 {
        let (r, g, b) = parse_rgb(s)?;
        return Some((r, g, b, 255));
    }
    if hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
    Some((r, g, b, a))
}

fn parse_csv_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parts[0].trim().parse().ok()?;
    let g = parts[1].trim().parse().ok()?;
    let b = parts[2].trim().parse().ok()?;
    let a = if parts.len() >= 4 {
        parts[3].trim().parse().ok()?
    } else {
        255
    };
    Some((r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_new() {
        let rgb = Rgb::new(10, 20, 30);
        assert_eq!(rgb.r, 10);
        assert_eq!(rgb.g, 20);
        assert_eq!(rgb.b, 30);
    }

    #[test]
    fn test_rgba_new() {
        let rgba = Rgba::new(10, 20, 30, 40);
        assert_eq!(rgba.r, 10);
        assert_eq!(rgba.g, 20);
        assert_eq!(rgba.b, 30);
        assert_eq!(rgba.a, 40);
    }

    #[test]
    fn test_parse_rgb_hex() {
        assert_eq!(parse_rgb("#1e1e23"), Some((0x1e, 0x1e, 0x23)));
        assert_eq!(parse_rgb("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(parse_rgb("#8a4fff"), Some((138, 79, 255)));
    }

    #[test]
    fn test_parse_rgb_csv() {
        assert_eq!(parse_rgb("30,30,35"), Some((30, 30, 35)));
        assert_eq!(parse_rgb("255, 255, 255"), Some((255, 255, 255)));
        assert_eq!(parse_rgb("  0,  0,  0  "), Some((0, 0, 0)));
    }

    #[test]
    fn test_parse_rgb_invalid() {
        assert_eq!(parse_rgb("#fff"), None);
        assert_eq!(parse_rgb("#gggggg"), None);
        assert_eq!(parse_rgb("not,a,color"), None);
        assert_eq!(parse_rgb(""), None);
    }

    #[test]
    fn test_parse_rgba_hex() {
        assert_eq!(parse_rgba("#1e1e23ff"), Some((0x1e, 0x1e, 0x23, 0xff)));
        assert_eq!(parse_rgba("#ffffff80"), Some((255, 255, 255, 128)));
        assert_eq!(parse_rgba("#00000000"), Some((0, 0, 0, 0)));
    }

    #[test]
    fn test_parse_rgba_hex_without_alpha() {
        assert_eq!(parse_rgba("#1e1e23"), Some((0x1e, 0x1e, 0x23, 255)));
    }

    #[test]
    fn test_parse_rgba_csv() {
        assert_eq!(parse_rgba("30,30,35,255"), Some((30, 30, 35, 255)));
        assert_eq!(parse_rgba("255, 255, 255, 128"), Some((255, 255, 255, 128)));
    }

    #[test]
    fn test_parse_rgba_csv_without_alpha() {
        assert_eq!(parse_rgba("30,30,35"), Some((30, 30, 35, 255)));
    }
}
