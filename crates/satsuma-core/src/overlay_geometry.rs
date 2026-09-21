//! Pure screen-rect/cursor placement math for the borderless wedge-menu
//! overlay window, kept OS-agnostic so it's testable without a Windows
//! toolchain.

/// A screen-space rectangle, e.g. the bounding box of a file icon in
/// Explorer. Platform-agnostic on purpose: the Windows-specific code
/// converts its native `RECT` into this before calling into shared logic,
/// so the placement math itself can be unit tested without a Windows
/// toolchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    /// X coordinate of the left edge.
    pub left: i32,
    /// Y coordinate of the top edge.
    pub top: i32,
    /// X coordinate of the right edge.
    pub right: i32,
    /// Y coordinate of the bottom edge.
    pub bottom: i32,
}

impl ScreenRect {
    /// Midpoint of the rectangle, rounded towards the top-left on odd
    /// dimensions (integer division).
    pub fn center(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
}

/// Top-left position for the wedge-menu overlay window: centered on the
/// selected file's icon when its rect is known, otherwise centered on the
/// current cursor position.
pub fn overlay_position(
    item_rect: Option<ScreenRect>,
    cursor: (i32, i32),
    overlay_size: (i32, i32),
) -> (i32, i32) {
    let (center_x, center_y) = item_rect.map(|rect| rect.center()).unwrap_or(cursor);
    (center_x - overlay_size.0 / 2, center_y - overlay_size.1 / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_on_the_item_rect_when_known() {
        let rect = ScreenRect { left: 100, top: 100, right: 140, bottom: 140 };
        assert_eq!(overlay_position(Some(rect), (0, 0), (320, 320)), (-40, -40));
    }

    #[test]
    fn falls_back_to_cursor_position_when_rect_unknown() {
        assert_eq!(overlay_position(None, (500, 300), (320, 320)), (340, 140));
    }

    #[test]
    fn rect_center_averages_corners() {
        let rect = ScreenRect { left: 10, top: 20, right: 30, bottom: 60 };
        assert_eq!(rect.center(), (20, 40));
    }
}
