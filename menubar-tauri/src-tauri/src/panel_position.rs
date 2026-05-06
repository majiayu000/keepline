//! Panel positioning relative to the tray icon, in physical pixels.
//!
//! Pure function so positioning behavior can be unit-tested on any host OS
//! without a live tray or monitor. Inputs and outputs are plain integers; the
//! caller (`lib.rs`) is responsible for converting between Tauri's
//! Logical/Physical units before and after.

#[derive(Debug, Clone, Copy)]
pub struct LayoutInput {
    pub tray_x: i32,
    pub tray_y: i32,
    pub tray_w: u32,
    pub tray_h: u32,
    pub panel_w: u32,
    pub panel_h: u32,
    pub work_x: i32,
    pub work_y: i32,
    pub work_w: u32,
    pub work_h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

const GAP: i32 = 4;

fn detect_edge(i: &LayoutInput) -> Edge {
    let cx = i.tray_x + i.tray_w as i32 / 2;
    let cy = i.tray_y + i.tray_h as i32 / 2;
    let d_top = (cy - i.work_y).abs();
    let d_bottom = (i.work_y + i.work_h as i32 - cy).abs();
    let d_left = (cx - i.work_x).abs();
    let d_right = (i.work_x + i.work_w as i32 - cx).abs();
    let min = d_top.min(d_bottom).min(d_left).min(d_right);
    if min == d_top {
        Edge::Top
    } else if min == d_bottom {
        Edge::Bottom
    } else if min == d_left {
        Edge::Left
    } else {
        Edge::Right
    }
}

pub fn compute_panel_position(i: LayoutInput) -> (i32, i32) {
    // Place the panel adjacent to the tray on the side opposite the screen
    // edge, then align it on the tray's center along the perpendicular axis.
    let center_x = i.tray_x + i.tray_w as i32 / 2 - i.panel_w as i32 / 2;
    let center_y = i.tray_y + i.tray_h as i32 / 2 - i.panel_h as i32 / 2;
    let (x, y) = match detect_edge(&i) {
        Edge::Top => (center_x, i.tray_y + i.tray_h as i32 + GAP),
        Edge::Bottom => (center_x, i.tray_y - i.panel_h as i32 - GAP),
        Edge::Left => (i.tray_x + i.tray_w as i32 + GAP, center_y),
        Edge::Right => (i.tray_x - i.panel_w as i32 - GAP, center_y),
    };

    let max_x = (i.work_x + i.work_w as i32 - i.panel_w as i32).max(i.work_x);
    let max_y = (i.work_y + i.work_h as i32 - i.panel_h as i32).max(i.work_y);
    (x.clamp(i.work_x, max_x), y.clamp(i.work_y, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(case: LayoutInput) -> (i32, i32) {
        compute_panel_position(case)
    }

    fn assert_inside_work_area(case: &LayoutInput, x: i32, y: i32) {
        assert!(
            x >= case.work_x && x + case.panel_w as i32 <= case.work_x + case.work_w as i32,
            "x={} out of work area x=[{}, {})",
            x,
            case.work_x,
            case.work_x + case.work_w as i32,
        );
        assert!(
            y >= case.work_y && y + case.panel_h as i32 <= case.work_y + case.work_h as i32,
            "y={} out of work area y=[{}, {})",
            y,
            case.work_y,
            case.work_y + case.work_h as i32,
        );
    }

    /// macOS top menubar — panel drops below the tray icon, centered on the
    /// tray's horizontal midpoint.
    #[test]
    fn macos_top_menubar_centers_panel_below_tray() {
        let case = LayoutInput {
            tray_x: 1200,
            tray_y: 0,
            tray_w: 22,
            tray_h: 22,
            panel_w: 360,
            panel_h: 480,
            work_x: 0,
            work_y: 25, // mac menubar takes ~25px
            work_w: 1920,
            work_h: 1055,
        };
        let (x, y) = run(case);
        // tray center x = 1200 + 11 = 1211; panel x = 1211 - 180 = 1031
        // panel y = tray_y + tray_h + GAP = 0 + 22 + 4 = 26
        assert_eq!(x, 1031);
        assert_eq!(y, 26);
        assert_inside_work_area(&case, x, y);
    }

    /// Issue #7: Windows 11, 2560x1440, taskbar at bottom-right.
    /// Old formula put y below tray (off-screen) and right edge clipped.
    /// New behavior must place panel above tray and inside work area.
    #[test]
    fn windows_bottom_right_taskbar_clamps_into_work_area() {
        let case = LayoutInput {
            tray_x: 2500,
            tray_y: 1410,
            tray_w: 24,
            tray_h: 24,
            panel_w: 380,
            panel_h: 520,
            work_x: 0,
            work_y: 0,
            work_w: 2560,
            work_h: 1400, // taskbar height 40px
        };
        let (x, y) = run(case);
        assert_inside_work_area(&case, x, y);
        // Panel must be above the tray icon, not below it.
        assert!(
            y + case.panel_h as i32 <= case.tray_y,
            "panel bottom {} must not overlap tray top {}",
            y + case.panel_h as i32,
            case.tray_y,
        );
        // Right edge flush against work area (clamped).
        assert_eq!(x, case.work_w as i32 - case.panel_w as i32);
    }

    /// Windows with vertical taskbar pinned to the left.
    #[test]
    fn windows_left_taskbar_panel_on_right_side() {
        let case = LayoutInput {
            tray_x: 0,
            tray_y: 700,
            tray_w: 48,
            tray_h: 32,
            panel_w: 380,
            panel_h: 520,
            work_x: 48,
            work_y: 0,
            work_w: 1872,
            work_h: 1080,
        };
        let (x, y) = run(case);
        assert_inside_work_area(&case, x, y);
        assert!(x >= case.tray_x + case.tray_w as i32, "panel must be to the right of tray");
    }

    /// Windows with vertical taskbar pinned to the right.
    #[test]
    fn windows_right_taskbar_panel_on_left_side() {
        let case = LayoutInput {
            tray_x: 2512,
            tray_y: 700,
            tray_w: 48,
            tray_h: 32,
            panel_w: 380,
            panel_h: 520,
            work_x: 0,
            work_y: 0,
            work_w: 2512,
            work_h: 1440,
        };
        let (x, y) = run(case);
        assert_inside_work_area(&case, x, y);
        assert!(
            x + case.panel_w as i32 <= case.tray_x,
            "panel right {} must not overlap tray left {}",
            x + case.panel_w as i32,
            case.tray_x,
        );
    }

    /// Multi-monitor: tray sits on a secondary monitor offset to the right.
    /// Work area origin is non-zero. Panel must stay inside that monitor.
    #[test]
    fn multi_monitor_secondary_display() {
        let case = LayoutInput {
            tray_x: 4400,
            tray_y: 1410,
            tray_w: 24,
            tray_h: 24,
            panel_w: 380,
            panel_h: 520,
            work_x: 2560,
            work_y: 0,
            work_w: 1920,
            work_h: 1400,
        };
        let (x, y) = run(case);
        assert_inside_work_area(&case, x, y);
        assert!(x >= 2560, "panel must stay on secondary monitor");
    }

    /// 200% DPI scaling: same shape as Windows bottom-right but doubled.
    /// Confirms the math has no scale-dependent assumptions.
    #[test]
    fn high_dpi_scales_proportionally() {
        let case = LayoutInput {
            tray_x: 5000,
            tray_y: 2820,
            tray_w: 48,
            tray_h: 48,
            panel_w: 760,
            panel_h: 1040,
            work_x: 0,
            work_y: 0,
            work_w: 5120,
            work_h: 2800,
        };
        let (x, y) = run(case);
        assert_inside_work_area(&case, x, y);
        assert!(y + case.panel_h as i32 <= case.tray_y);
    }

    /// Pathological: panel taller than work area. Clamp must still produce
    /// a coordinate inside the work area origin (no panic, no negative).
    #[test]
    fn panel_larger_than_work_area_does_not_overflow() {
        let case = LayoutInput {
            tray_x: 100,
            tray_y: 100,
            tray_w: 24,
            tray_h: 24,
            panel_w: 400,
            panel_h: 400,
            work_x: 0,
            work_y: 0,
            work_w: 200,
            work_h: 200,
        };
        let (x, y) = run(case);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }
}
