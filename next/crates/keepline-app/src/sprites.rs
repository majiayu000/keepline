//! Cell-grid geometry, following Ghostty sprite/draw: no font side bearings.
use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, fill, point, px, size};
#[derive(Debug)]
enum Shape {
    Shade(f32),
    Rect(f32, f32, f32, f32),
    Path(Vec<(f32, f32)>, Option<f32>),
}
fn geometry(c: char, w: f32, h: f32, t: f32) -> Option<Vec<Shape>> {
    let cp = c as u32;
    let mut out = vec![];
    let mut rect = |x, y, r, b| out.push(Shape::Rect(x, y, r, b));
    match cp {
        0x2580 => rect(0., 0., w, h / 2.),
        0x2581..=0x2588 => rect(0., h * (1. - (cp - 0x2580) as f32 / 8.), w, h),
        0x2589..=0x258f => rect(0., 0., w * (0x2590 - cp) as f32 / 8., h),
        0x2590 => rect(w / 2., 0., w, h),
        0x2591..=0x2593 => out.push(Shape::Shade((cp - 0x2590) as f32 / 4.)),
        0x2594 => rect(0., 0., w, h / 8.),
        0x2595 => rect(w * 7. / 8., 0., w, h),
        0x2596..=0x259f => {
            let mask = [4, 8, 1, 13, 9, 7, 11, 2, 6, 14][(cp - 0x2596) as usize];
            for i in 0..4 {
                if mask & (1 << i) != 0 {
                    let x = (i % 2) as f32 * w / 2.;
                    let y = (i / 2) as f32 * h / 2.;
                    rect(x, y, x + w / 2., y + h / 2.);
                }
            }
        }
        0x2800..=0x28ff => {
            // Same eight-dot bit order as Unicode/Ghostty. Fit dot size, spacing,
            // and margins to physical cell pixels, then map back to points.
            let bits = cp - 0x2800;
            let (w, h) = (w as i32, h as i32);
            let mut d = (w / 4).min(h / 8);
            let (mut xs, mut ys) = (w / 4, h / 8);
            let (mut xm, mut ym) = (xs / 2, ys / 2);
            let (mut xr, mut yr) = (w - 2 * xm - xs - 2 * d, h - 2 * ym - 3 * ys - 4 * d);
            if xr >= 2 && yr >= 4 && d == 0 {
                d += 1;
                xr -= 2;
                yr -= 4;
            }
            if xr >= 2 && xm == 0 {
                xm = 1;
                xr -= 2;
            }
            if yr >= 2 && ym == 0 {
                ym = 1;
                yr -= 2;
            }
            if xr >= 1 {
                xs += 1;
                xr -= 1;
            }
            if yr >= 3 {
                ys += 1;
                yr -= 3;
            }
            if xr >= 2 {
                xm += 1;
                xr -= 2;
            }
            if yr >= 2 {
                ym += 1;
                yr -= 2;
            }
            if xr >= 2 && yr >= 4 {
                d += 1;
            }
            for (i, (x, y)) in [
                (0, 0),
                (0, 1),
                (0, 2),
                (1, 0),
                (1, 1),
                (1, 2),
                (0, 3),
                (1, 3),
            ]
            .into_iter()
            .enumerate()
            {
                if bits & (1 << i) != 0 {
                    let x = (xm + x * (d + xs)) as f32;
                    let y = (ym + y * (d + ys)) as f32;
                    rect(x, y, x + d as f32, y + d as f32);
                }
            }
        }
        0x2500..=0x257f => {
            if (0x2571..=0x2573).contains(&cp) {
                if cp != 0x2572 {
                    out.push(Shape::Path(vec![(0., h), (w, 0.)], Some(t)));
                }
                if cp != 0x2571 {
                    out.push(Shape::Path(vec![(0., 0.), (w, h)], Some(t)));
                }
            } else if (0x256d..=0x2570).contains(&cp) {
                // Round corners touch both cell edges; radius is half the narrower dimension.
                let (flip_x, flip_y) = match cp {
                    0x256d => (false, false),
                    0x256e => (true, false),
                    0x256f => (true, true),
                    _ => (false, true),
                };
                let r = (w / 2.).min(h / 2.);
                let mut pts = vec![(w, h / 2.), (w / 2. + r, h / 2.)];
                for i in 0..=12 {
                    let a =
                        std::f32::consts::PI * 1.5 - i as f32 / 12. * std::f32::consts::FRAC_PI_2;
                    pts.push((w / 2. + r + r * a.cos(), h / 2. + r + r * a.sin()));
                }
                pts.push((w / 2., h));
                for p in &mut pts {
                    if flip_x {
                        p.0 = w - p.0;
                    }
                    if flip_y {
                        p.1 = h - p.1;
                    }
                }
                out.push(Shape::Path(pts, Some(t)));
            } else {
                let arms = BOX_ARMS[(cp - 0x2500) as usize];
                let dashes = match cp {
                    0x2504..=0x2507 => 3,
                    0x2508..=0x250b => 4,
                    0x254c..=0x254f => 2,
                    _ => 0,
                };
                if dashes != 0 {
                    let horizontal = arms[0] != 0;
                    let len = if horizontal { w } else { h };
                    let thick = t * if arms.iter().any(|x| *x == 2) { 2. } else { 1. };
                    for i in 0..dashes {
                        let start = len * i as f32 / dashes as f32;
                        let end = start + len / dashes as f32 * 0.7;
                        if horizontal {
                            rect(start, h / 2. - thick / 2., end, h / 2. + thick / 2.);
                        } else {
                            rect(w / 2. - thick / 2., start, w / 2. + thick / 2., end);
                        }
                    }
                } else {
                    for (i, weight) in arms.into_iter().enumerate() {
                        if weight == 0 {
                            continue;
                        }
                        let offsets: &[f32] = if weight == 3 { &[-1., 1.] } else { &[0.] };
                        let thick = if weight == 2 { 2. * t } else { t };
                        for off in offsets {
                            let shift = off * t;
                            match i {
                                0 => rect(
                                    0.,
                                    h / 2. + shift - thick / 2.,
                                    w / 2. + thick / 2.,
                                    h / 2. + shift + thick / 2.,
                                ),
                                1 => rect(
                                    w / 2. - thick / 2.,
                                    h / 2. + shift - thick / 2.,
                                    w,
                                    h / 2. + shift + thick / 2.,
                                ),
                                2 => rect(
                                    w / 2. + shift - thick / 2.,
                                    0.,
                                    w / 2. + shift + thick / 2.,
                                    h / 2. + thick / 2.,
                                ),
                                _ => rect(
                                    w / 2. + shift - thick / 2.,
                                    h / 2. - thick / 2.,
                                    w / 2. + shift + thick / 2.,
                                    h,
                                ),
                            }
                        }
                    }
                }
            }
        }
        0xe0b0..=0xe0bf => {
            let stroke = (cp % 2 == 1).then_some(t);
            let mut p = match cp {
                0xe0b0 | 0xe0b1 => vec![(0., 0.), (w, h / 2.), (0., h)],
                0xe0b2 | 0xe0b3 => vec![(w, 0.), (0., h / 2.), (w, h)],
                0xe0b4..=0xe0b7 => {
                    let left = cp >= 0xe0b6;
                    (0..=24)
                        .map(|i| {
                            let a = -std::f32::consts::FRAC_PI_2
                                + i as f32 / 24. * std::f32::consts::PI;
                            let x = w * a.cos();
                            (if left { w - x } else { x }, h / 2. + h / 2. * a.sin())
                        })
                        .collect()
                }
                0xe0b8 => vec![(0., 0.), (w, h), (0., h)],
                0xe0ba => vec![(w, 0.), (w, h), (0., h)],
                0xe0bc => vec![(0., 0.), (w, 0.), (0., h)],
                0xe0be => vec![(0., 0.), (w, 0.), (w, h)],
                0xe0b9 | 0xe0bf => vec![(0., 0.), (w, h)],
                _ => vec![(w, 0.), (0., h)],
            };
            if stroke.is_none() {
                p.push(p[0]);
            }
            out.push(Shape::Path(p, stroke));
        }
        _ => return None,
    }
    Some(out)
}
pub fn paint(
    c: char,
    origin: Point<Pixels>,
    width: Pixels,
    height: Pixels,
    stroke: Pixels,
    color: Hsla,
    window: &mut Window,
) -> bool {
    let scale = window.scale_factor();
    let Some(shapes) = geometry(
        c,
        (f32::from(width) * scale).round(),
        (f32::from(height) * scale).round(),
        f32::from(stroke) * scale,
    ) else {
        return false;
    };
    for shape in shapes {
        match shape {
            Shape::Shade(alpha) => window.paint_quad(fill(
                Bounds::new(origin, size(width, height)),
                color.opacity(alpha),
            )),
            Shape::Rect(x, y, r, b) => window.paint_quad(fill(
                Bounds::new(
                    origin + point(px(x / scale), px(y / scale)),
                    size(px((r - x) / scale), px((b - y) / scale)),
                ),
                color,
            )),
            Shape::Path(points, stroke) => {
                let mut p = if let Some(t) = stroke {
                    PathBuilder::stroke(px(t / scale))
                } else {
                    PathBuilder::fill()
                };
                for (i, (x, y)) in points.into_iter().enumerate() {
                    let pos = origin + point(px(x / scale), px(y / scale));
                    if i == 0 {
                        p.move_to(pos);
                    } else {
                        p.line_to(pos);
                    }
                }
                if stroke.is_none() {
                    p.close();
                }
                if let Ok(path) = p.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_fill_cell_and_braille_bits_are_not_font_spaced() {
        assert!(matches!(
            &geometry('█', 8., 16., 1.).unwrap()[0],
            Shape::Rect(0., 0., 8., 16.)
        ));
        assert_eq!(geometry('⣿', 8., 16., 1.).unwrap().len(), 8);
        assert_eq!(geometry('⠀', 8., 16., 1.).unwrap().len(), 0);
        assert_eq!(geometry('▚', 8., 16., 1.).unwrap().len(), 2);
    }
    #[test]
    fn all_box_and_powerline_shapes_are_defined_and_touch_edges() {
        for cp in (0x2500..=0x257f).chain(0xe0b0..=0xe0bf) {
            assert!(
                !geometry(char::from_u32(cp).unwrap(), 8., 16., 1.)
                    .unwrap()
                    .is_empty(),
                "{cp:x}"
            );
        }
        assert!(matches!(
            &geometry('│', 8., 16., 1.).unwrap()[0],
            Shape::Rect(_, 0., _, _)
        ));
        assert!(
            matches!(&geometry('',8.,16.,1.).unwrap()[0],Shape::Path(p,None) if p.contains(&(0.,16.)) && p.contains(&(8.,8.)))
        );
    }
}

// Unicode U+2500..U+257F arms: left, right, up, down; 1 light, 2 heavy, 3 double.
const BOX_ARMS: [[u8; 4]; 128] = [
    [1, 1, 0, 0], // ─ LIGHT HORIZONTAL
    [2, 2, 0, 0], // ━ HEAVY HORIZONTAL
    [0, 0, 1, 1], // │ LIGHT VERTICAL
    [0, 0, 2, 2], // ┃ HEAVY VERTICAL
    [1, 1, 0, 0], // ┄ LIGHT TRIPLE DASH HORIZONTAL
    [2, 2, 0, 0], // ┅ HEAVY TRIPLE DASH HORIZONTAL
    [0, 0, 1, 1], // ┆ LIGHT TRIPLE DASH VERTICAL
    [0, 0, 2, 2], // ┇ HEAVY TRIPLE DASH VERTICAL
    [1, 1, 0, 0], // ┈ LIGHT QUADRUPLE DASH HORIZONTAL
    [2, 2, 0, 0], // ┉ HEAVY QUADRUPLE DASH HORIZONTAL
    [0, 0, 1, 1], // ┊ LIGHT QUADRUPLE DASH VERTICAL
    [0, 0, 2, 2], // ┋ HEAVY QUADRUPLE DASH VERTICAL
    [0, 1, 0, 1], // ┌ LIGHT DOWN AND RIGHT
    [0, 2, 0, 1], // ┍ DOWN LIGHT AND RIGHT HEAVY
    [0, 1, 0, 2], // ┎ DOWN HEAVY AND RIGHT LIGHT
    [0, 2, 0, 2], // ┏ HEAVY DOWN AND RIGHT
    [1, 0, 0, 1], // ┐ LIGHT DOWN AND LEFT
    [2, 0, 0, 1], // ┑ DOWN LIGHT AND LEFT HEAVY
    [1, 0, 0, 2], // ┒ DOWN HEAVY AND LEFT LIGHT
    [2, 0, 0, 2], // ┓ HEAVY DOWN AND LEFT
    [0, 1, 1, 0], // └ LIGHT UP AND RIGHT
    [0, 2, 1, 0], // ┕ UP LIGHT AND RIGHT HEAVY
    [0, 1, 2, 0], // ┖ UP HEAVY AND RIGHT LIGHT
    [0, 2, 2, 0], // ┗ HEAVY UP AND RIGHT
    [1, 0, 1, 0], // ┘ LIGHT UP AND LEFT
    [2, 0, 1, 0], // ┙ UP LIGHT AND LEFT HEAVY
    [1, 0, 2, 0], // ┚ UP HEAVY AND LEFT LIGHT
    [2, 0, 2, 0], // ┛ HEAVY UP AND LEFT
    [0, 1, 1, 1], // ├ LIGHT VERTICAL AND RIGHT
    [0, 2, 1, 1], // ┝ VERTICAL LIGHT AND RIGHT HEAVY
    [0, 1, 2, 1], // ┞ UP HEAVY AND RIGHT DOWN LIGHT
    [0, 1, 1, 2], // ┟ DOWN HEAVY AND RIGHT UP LIGHT
    [0, 1, 2, 2], // ┠ VERTICAL HEAVY AND RIGHT LIGHT
    [0, 2, 2, 1], // ┡ DOWN LIGHT AND RIGHT UP HEAVY
    [0, 2, 1, 2], // ┢ UP LIGHT AND RIGHT DOWN HEAVY
    [0, 2, 2, 2], // ┣ HEAVY VERTICAL AND RIGHT
    [1, 0, 1, 1], // ┤ LIGHT VERTICAL AND LEFT
    [2, 0, 1, 1], // ┥ VERTICAL LIGHT AND LEFT HEAVY
    [1, 0, 2, 1], // ┦ UP HEAVY AND LEFT DOWN LIGHT
    [1, 0, 1, 2], // ┧ DOWN HEAVY AND LEFT UP LIGHT
    [1, 0, 2, 2], // ┨ VERTICAL HEAVY AND LEFT LIGHT
    [2, 0, 2, 1], // ┩ DOWN LIGHT AND LEFT UP HEAVY
    [2, 0, 1, 2], // ┪ UP LIGHT AND LEFT DOWN HEAVY
    [2, 0, 2, 2], // ┫ HEAVY VERTICAL AND LEFT
    [1, 1, 0, 1], // ┬ LIGHT DOWN AND HORIZONTAL
    [2, 1, 0, 1], // ┭ LEFT HEAVY AND RIGHT DOWN LIGHT
    [1, 2, 0, 1], // ┮ RIGHT HEAVY AND LEFT DOWN LIGHT
    [2, 2, 0, 1], // ┯ DOWN LIGHT AND HORIZONTAL HEAVY
    [1, 1, 0, 2], // ┰ DOWN HEAVY AND HORIZONTAL LIGHT
    [2, 1, 0, 2], // ┱ RIGHT LIGHT AND LEFT DOWN HEAVY
    [1, 2, 0, 2], // ┲ LEFT LIGHT AND RIGHT DOWN HEAVY
    [2, 2, 0, 2], // ┳ HEAVY DOWN AND HORIZONTAL
    [1, 1, 1, 0], // ┴ LIGHT UP AND HORIZONTAL
    [2, 1, 1, 0], // ┵ LEFT HEAVY AND RIGHT UP LIGHT
    [1, 2, 1, 0], // ┶ RIGHT HEAVY AND LEFT UP LIGHT
    [2, 2, 1, 0], // ┷ UP LIGHT AND HORIZONTAL HEAVY
    [1, 1, 2, 0], // ┸ UP HEAVY AND HORIZONTAL LIGHT
    [2, 1, 2, 0], // ┹ RIGHT LIGHT AND LEFT UP HEAVY
    [1, 2, 2, 0], // ┺ LEFT LIGHT AND RIGHT UP HEAVY
    [2, 2, 2, 0], // ┻ HEAVY UP AND HORIZONTAL
    [1, 1, 1, 1], // ┼ LIGHT VERTICAL AND HORIZONTAL
    [2, 1, 1, 1], // ┽ LEFT HEAVY AND RIGHT VERTICAL LIGHT
    [1, 2, 1, 1], // ┾ RIGHT HEAVY AND LEFT VERTICAL LIGHT
    [2, 2, 1, 1], // ┿ VERTICAL LIGHT AND HORIZONTAL HEAVY
    [1, 1, 2, 1], // ╀ UP HEAVY AND DOWN HORIZONTAL LIGHT
    [1, 1, 1, 2], // ╁ DOWN HEAVY AND UP HORIZONTAL LIGHT
    [1, 1, 2, 2], // ╂ VERTICAL HEAVY AND HORIZONTAL LIGHT
    [2, 1, 2, 1], // ╃ LEFT UP HEAVY AND RIGHT DOWN LIGHT
    [1, 2, 2, 1], // ╄ RIGHT UP HEAVY AND LEFT DOWN LIGHT
    [2, 1, 1, 2], // ╅ LEFT DOWN HEAVY AND RIGHT UP LIGHT
    [1, 2, 1, 2], // ╆ RIGHT DOWN HEAVY AND LEFT UP LIGHT
    [2, 2, 2, 1], // ╇ DOWN LIGHT AND UP HORIZONTAL HEAVY
    [2, 2, 1, 2], // ╈ UP LIGHT AND DOWN HORIZONTAL HEAVY
    [2, 1, 2, 2], // ╉ RIGHT LIGHT AND LEFT VERTICAL HEAVY
    [1, 2, 2, 2], // ╊ LEFT LIGHT AND RIGHT VERTICAL HEAVY
    [2, 2, 2, 2], // ╋ HEAVY VERTICAL AND HORIZONTAL
    [1, 1, 0, 0], // ╌ LIGHT DOUBLE DASH HORIZONTAL
    [2, 2, 0, 0], // ╍ HEAVY DOUBLE DASH HORIZONTAL
    [0, 0, 1, 1], // ╎ LIGHT DOUBLE DASH VERTICAL
    [0, 0, 2, 2], // ╏ HEAVY DOUBLE DASH VERTICAL
    [3, 3, 0, 0], // ═ DOUBLE HORIZONTAL
    [0, 0, 3, 3], // ║ DOUBLE VERTICAL
    [0, 3, 0, 1], // ╒ DOWN SINGLE AND RIGHT DOUBLE
    [0, 1, 0, 3], // ╓ DOWN DOUBLE AND RIGHT SINGLE
    [0, 3, 0, 3], // ╔ DOUBLE DOWN AND RIGHT
    [3, 0, 0, 1], // ╕ DOWN SINGLE AND LEFT DOUBLE
    [1, 0, 0, 3], // ╖ DOWN DOUBLE AND LEFT SINGLE
    [3, 0, 0, 3], // ╗ DOUBLE DOWN AND LEFT
    [0, 3, 1, 0], // ╘ UP SINGLE AND RIGHT DOUBLE
    [0, 1, 3, 0], // ╙ UP DOUBLE AND RIGHT SINGLE
    [0, 3, 3, 0], // ╚ DOUBLE UP AND RIGHT
    [3, 0, 1, 0], // ╛ UP SINGLE AND LEFT DOUBLE
    [1, 0, 3, 0], // ╜ UP DOUBLE AND LEFT SINGLE
    [3, 0, 3, 0], // ╝ DOUBLE UP AND LEFT
    [0, 3, 1, 1], // ╞ VERTICAL SINGLE AND RIGHT DOUBLE
    [0, 1, 3, 3], // ╟ VERTICAL DOUBLE AND RIGHT SINGLE
    [0, 3, 3, 3], // ╠ DOUBLE VERTICAL AND RIGHT
    [3, 0, 1, 1], // ╡ VERTICAL SINGLE AND LEFT DOUBLE
    [1, 0, 3, 3], // ╢ VERTICAL DOUBLE AND LEFT SINGLE
    [3, 0, 3, 3], // ╣ DOUBLE VERTICAL AND LEFT
    [3, 3, 0, 1], // ╤ DOWN SINGLE AND HORIZONTAL DOUBLE
    [1, 1, 0, 3], // ╥ DOWN DOUBLE AND HORIZONTAL SINGLE
    [3, 3, 0, 3], // ╦ DOUBLE DOWN AND HORIZONTAL
    [3, 3, 1, 0], // ╧ UP SINGLE AND HORIZONTAL DOUBLE
    [1, 1, 3, 0], // ╨ UP DOUBLE AND HORIZONTAL SINGLE
    [3, 3, 3, 0], // ╩ DOUBLE UP AND HORIZONTAL
    [3, 3, 1, 1], // ╪ VERTICAL SINGLE AND HORIZONTAL DOUBLE
    [1, 1, 3, 3], // ╫ VERTICAL DOUBLE AND HORIZONTAL SINGLE
    [3, 3, 3, 3], // ╬ DOUBLE VERTICAL AND HORIZONTAL
    [0, 1, 0, 1], // ╭ LIGHT ARC DOWN AND RIGHT
    [1, 0, 0, 1], // ╮ LIGHT ARC DOWN AND LEFT
    [1, 0, 1, 0], // ╯ LIGHT ARC UP AND LEFT
    [0, 1, 1, 0], // ╰ LIGHT ARC UP AND RIGHT
    [1, 1, 0, 0], // ╱ LIGHT DIAGONAL UPPER RIGHT TO LOWER LEFT
    [1, 1, 0, 0], // ╲ LIGHT DIAGONAL UPPER LEFT TO LOWER RIGHT
    [0, 0, 0, 0], // ╳ LIGHT DIAGONAL CROSS
    [1, 0, 0, 0], // ╴ LIGHT LEFT
    [0, 0, 1, 0], // ╵ LIGHT UP
    [0, 1, 0, 0], // ╶ LIGHT RIGHT
    [0, 0, 0, 1], // ╷ LIGHT DOWN
    [2, 0, 0, 0], // ╸ HEAVY LEFT
    [0, 0, 2, 0], // ╹ HEAVY UP
    [0, 2, 0, 0], // ╺ HEAVY RIGHT
    [0, 0, 0, 2], // ╻ HEAVY DOWN
    [1, 2, 0, 0], // ╼ LIGHT LEFT AND HEAVY RIGHT
    [0, 0, 1, 2], // ╽ LIGHT UP AND HEAVY DOWN
    [2, 1, 0, 0], // ╾ HEAVY LEFT AND LIGHT RIGHT
    [0, 0, 2, 1], // ╿ HEAVY UP AND LIGHT DOWN
];
