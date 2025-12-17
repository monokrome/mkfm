//! Overlay configuration

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverlayPosition {
    #[default]
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Clone, Debug)]
pub enum Dimension {
    Pixels(i32),
    Percent(f32),
}

impl Dimension {
    pub fn resolve(&self, reference: u32) -> i32 {
        match self {
            Dimension::Pixels(px) => *px,
            Dimension::Percent(pct) => (reference as f32 * pct / 100.0) as i32,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(stripped) = s.strip_suffix('%') {
            stripped.parse::<f32>().ok().map(Dimension::Percent)
        } else {
            s.parse::<i32>().ok().map(Dimension::Pixels)
        }
    }
}

#[derive(Clone, Debug)]
pub struct OverlayConfig {
    pub enabled: bool,
    pub margin: Dimension,
    pub position: OverlayPosition,
    pub offset: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            margin: Dimension::Percent(2.0),
            position: OverlayPosition::Right,
            offset: Dimension::Pixels(0),
            max_width: Dimension::Pixels(400),
            max_height: Dimension::Percent(100.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_pixels() {
        let dim = Dimension::Pixels(100);
        assert_eq!(dim.resolve(1000), 100);
        assert_eq!(dim.resolve(500), 100);
    }

    #[test]
    fn test_dimension_percent() {
        let dim = Dimension::Percent(50.0);
        assert_eq!(dim.resolve(1000), 500);
        assert_eq!(dim.resolve(200), 100);
    }

    #[test]
    fn test_dimension_parse_pixels() {
        match Dimension::parse("100") {
            Some(Dimension::Pixels(px)) => assert_eq!(px, 100),
            _ => panic!("expected Pixels(100)"),
        }
        match Dimension::parse("  200  ") {
            Some(Dimension::Pixels(px)) => assert_eq!(px, 200),
            _ => panic!("expected Pixels(200)"),
        }
    }

    #[test]
    fn test_dimension_parse_percent() {
        match Dimension::parse("50%") {
            Some(Dimension::Percent(pct)) => assert!((pct - 50.0).abs() < 0.001),
            _ => panic!("expected Percent(50.0)"),
        }
        match Dimension::parse("  75.5%  ") {
            Some(Dimension::Percent(pct)) => assert!((pct - 75.5).abs() < 0.001),
            _ => panic!("expected Percent(75.5)"),
        }
    }

    #[test]
    fn test_dimension_parse_invalid() {
        assert!(Dimension::parse("abc").is_none());
        assert!(Dimension::parse("").is_none());
    }

    #[test]
    fn test_overlay_position_default() {
        assert_eq!(OverlayPosition::default(), OverlayPosition::Right);
    }

    #[test]
    fn test_overlay_config_default() {
        let config = OverlayConfig::default();
        assert!(config.enabled);
        assert_eq!(config.position, OverlayPosition::Right);
    }
}
