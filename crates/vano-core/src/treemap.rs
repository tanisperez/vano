//! The squarified treemap layout (Bruls, Huizing and van Wijk), the same algorithm as the
//! design's prototype: rows of tiles laid along the shorter side, each row kept while it
//! improves the worst aspect ratio.

/// A rectangle in layout space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The rectangle shrunk by `by` on every side, never below zero size.
    pub fn inset(self, by: f32) -> Self {
        self.inset_sides(by, by, by, by)
    }

    /// The rectangle shrunk by different amounts per side, never below zero size.
    pub fn inset_sides(self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            x: self.x + left,
            y: self.y + top,
            w: (self.w - left - right).max(0.0),
            h: (self.h - top - bottom).max(0.0),
        }
    }

    /// Linear interpolation between `self` (t = 0) and `to` (t = 1).
    pub fn lerp(self, to: Rect, t: f32) -> Rect {
        Rect {
            x: self.x + (to.x - self.x) * t,
            y: self.y + (to.y - self.y) * t,
            w: self.w + (to.w - self.w) * t,
            h: self.h + (to.h - self.h) * t,
        }
    }

    /// The rectangle with its center kept and its size set to zero.
    pub fn collapsed(self) -> Rect {
        Rect::new(self.x + self.w / 2.0, self.y + self.h / 2.0, 0.0, 0.0)
    }

    /// Maps `self`, given in the coordinates of `from`, into `to`: the transform that turns
    /// `from` into `to`, applied to `self`. Used by the zoom animation.
    pub fn map(self, from: Rect, to: Rect) -> Rect {
        let sx = if from.w > 0.0 { to.w / from.w } else { 1.0 };
        let sy = if from.h > 0.0 { to.h / from.h } else { 1.0 };
        Rect {
            x: to.x + (self.x - from.x) * sx,
            y: to.y + (self.y - from.y) * sy,
            w: self.w * sx,
            h: self.h * sy,
        }
    }

    pub fn area(self) -> f32 {
        self.w * self.h
    }
}

/// Lays out `values` inside `rect`. Returns one rectangle per value, in the same order as
/// `values`. Values do not need to be sorted; zero or negative values get a collapsed rectangle
/// at the center.
pub fn squarify(values: &[f64], rect: Rect) -> Vec<Rect> {
    let mut out = vec![rect.collapsed(); values.len()];
    let mut order: Vec<usize> = (0..values.len()).filter(|i| values[*i] > 0.0).collect();
    order.sort_by(|a, b| values[*b].total_cmp(&values[*a]));
    let total: f64 = order.iter().map(|i| values[*i]).sum();
    if total <= 0.0 || rect.w <= 0.0 || rect.h <= 0.0 {
        return out;
    }
    let scale = f64::from(rect.w) * f64::from(rect.h) / total;
    let areas: Vec<f64> = order.iter().map(|i| values[*i] * scale).collect();

    let (mut x, mut y, mut w, mut h) = (
        f64::from(rect.x),
        f64::from(rect.y),
        f64::from(rect.w),
        f64::from(rect.h),
    );
    let mut start = 0;
    while start < areas.len() {
        let side = w.min(h);
        let mut end = start + 1;
        let mut sum = areas[start];
        let mut current = worst(&areas[start..end], sum, side);
        while end < areas.len() {
            let next_sum = sum + areas[end];
            let next = worst(&areas[start..=end], next_sum, side);
            if next > current {
                break;
            }
            sum = next_sum;
            current = next;
            end += 1;
        }
        // Lay the row [start, end) along the shorter side.
        if w >= h {
            let col_w = if h > 0.0 { sum / h } else { 0.0 };
            let mut cy = y;
            for k in start..end {
                let eh = if col_w > 0.0 { areas[k] / col_w } else { 0.0 };
                out[order[k]] = Rect::new(x as f32, cy as f32, col_w as f32, eh as f32);
                cy += eh;
            }
            x += col_w;
            w = (w - col_w).max(0.0);
        } else {
            let row_h = if w > 0.0 { sum / w } else { 0.0 };
            let mut cx = x;
            for k in start..end {
                let ew = if row_h > 0.0 { areas[k] / row_h } else { 0.0 };
                out[order[k]] = Rect::new(cx as f32, y as f32, ew as f32, row_h as f32);
                cx += ew;
            }
            y += row_h;
            h = (h - row_h).max(0.0);
        }
        start = end;
    }
    out
}

/// The worst aspect ratio of a row with these areas, laid along a side of length `side`.
fn worst(row: &[f64], sum: f64, side: f64) -> f64 {
    let (mut max, mut min) = (0f64, f64::INFINITY);
    for a in row {
        max = max.max(*a);
        min = min.min(*a);
    }
    if sum <= 0.0 || min <= 0.0 {
        return f64::INFINITY;
    }
    let s2 = side * side;
    let sum2 = sum * sum;
    (s2 * max / sum2).max(sum2 / (s2 * min))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_the_rect_with_proportional_areas() {
        let rect = Rect::new(0.0, 0.0, 600.0, 400.0);
        let values = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
        let out = squarify(&values, rect);
        let total: f64 = values.iter().sum();
        for (v, r) in values.iter().zip(&out) {
            let expected = v / total * 240_000.0;
            assert!((f64::from(r.area()) - expected).abs() < 1.0, "{r:?}");
            assert!(r.x >= -0.01 && r.y >= -0.01);
            assert!(r.x + r.w <= 600.01 && r.y + r.h <= 400.01);
        }
    }

    #[test]
    fn zero_values_collapse() {
        let out = squarify(&[0.0, 5.0], Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(out[0].w, 0.0);
        assert_eq!(out[1], Rect::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn map_transforms_between_frames() {
        let from = Rect::new(10.0, 10.0, 100.0, 50.0);
        let to = Rect::new(0.0, 0.0, 200.0, 200.0);
        let r = Rect::new(60.0, 35.0, 50.0, 25.0).map(from, to);
        assert_eq!(r, Rect::new(100.0, 100.0, 100.0, 100.0));
    }
}
