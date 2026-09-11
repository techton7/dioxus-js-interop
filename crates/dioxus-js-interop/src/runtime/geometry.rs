use serde::{Deserialize, Serialize};

/// Standard 2D bounding rectangle with sub-pixel floating-point coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub top: f64,
    #[serde(default)]
    pub right: f64,
    #[serde(default)]
    pub bottom: f64,
    #[serde(default)]
    pub left: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            top: y,
            left: x,
            right: x + width,
            bottom: y + height,
        }
    }
}


/// Viewport dimensions and scroll offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    #[serde(default, alias = "scroll_x")]
    pub scroll_x: f64,
    #[serde(default, alias = "scroll_y")]
    pub scroll_y: f64,
}

/// 2D point coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// 2D dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_serde_roundtrip() {
        let json = r#"{"x":10.5,"y":20.0,"width":100.0,"height":50.0,"top":20.0,"right":110.5,"bottom":70.0,"left":10.5}"#;
        let rect: Rect = serde_json::from_str(json).unwrap();
        assert_eq!(rect.x, 10.5);
        assert_eq!(rect.y, 20.0);
        assert_eq!(rect.width, 100.0);
        assert_eq!(rect.height, 50.0);
        assert_eq!(rect.top, 20.0);
        assert_eq!(rect.right, 110.5);
        assert_eq!(rect.bottom, 70.0);
        assert_eq!(rect.left, 10.5);
    }

    #[test]
    fn test_rect_serde_defaults() {
        let json = r#"{"x":10.0,"y":20.0,"width":100.0,"height":50.0}"#;
        let rect: Rect = serde_json::from_str(json).unwrap();
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.top, 0.0); // serde default
    }

    #[test]
    fn test_viewport_serde_roundtrip() {
        let json = r#"{"width":1920.0,"height":1080.0,"scrollX":0.0,"scrollY":150.0}"#;
        let vp: Viewport = serde_json::from_str(json).unwrap();
        assert_eq!(vp.width, 1920.0);
        assert_eq!(vp.height, 1080.0);
        assert_eq!(vp.scroll_x, 0.0);
        assert_eq!(vp.scroll_y, 150.0);

        // Backwards compatibility with snake_case alias
        let legacy_json = r#"{"width":1920.0,"height":1080.0,"scroll_x":10.0,"scroll_y":200.0}"#;
        let legacy_vp: Viewport = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy_vp.scroll_x, 10.0);
        assert_eq!(legacy_vp.scroll_y, 200.0);
    }
}
