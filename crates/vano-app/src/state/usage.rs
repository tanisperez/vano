//! The figures behind the disk usage bars: how full the disk is now and after the selected
//! cleanup, and how a bar's gradient is cut to its filled part.

use vano_scan::DiskUsage;

/// Where the colors of a disk usage bar's gradient sit along the whole track. The gradient
/// belongs to the track, not to the filled part, so a fuller disk reaches further into the red.
pub const GRADIENT_STOPS: [f32; 5] = [0.0, 0.35, 0.62, 0.82, 1.0];

/// How worrying a fill level is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Low,
    Mid,
    High,
}

impl Severity {
    /// The severity of a disk filled `fraction` of the way: from 60% and from 85%.
    pub fn of(fraction: f32) -> Self {
        if fraction >= 0.85 {
            Self::High
        } else if fraction >= 0.60 {
            Self::Mid
        } else {
            Self::Low
        }
    }
}

/// A disk as it is, and as it would be once the selected bytes are gone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsagePreview {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub used_after: u64,
    pub free_after: u64,
    /// Bytes the preview really takes off, never more than the disk uses.
    pub freed: u64,
}

impl UsagePreview {
    pub fn new(disk: DiskUsage, selected: u64) -> Self {
        let used = disk.used();
        let freed = selected.min(used);
        Self {
            total: disk.total,
            used,
            free: disk.available,
            used_after: used - freed,
            free_after: disk.available.saturating_add(freed),
            freed,
        }
    }

    /// How full the disk is now, from 0 to 1.
    pub fn fraction(&self) -> f32 {
        self.of_total(self.used)
    }

    /// How full the disk would be after the cleanup, from 0 to 1.
    pub fn fraction_after(&self) -> f32 {
        self.of_total(self.used_after)
    }

    fn of_total(&self, bytes: u64) -> f32 {
        (bytes as f64 / self.total.max(1) as f64).clamp(0.0, 1.0) as f32
    }
}

/// One piece of a filled bar, between two neighbouring gradient stops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Index of the stop it starts at, into [`GRADIENT_STOPS`].
    pub stop: usize,
    /// Where it starts and ends along the track, from 0 to 1.
    pub from: f32,
    pub to: f32,
    /// How far it reaches towards the next stop's color, 1 unless the fill ends inside it.
    pub reach: f32,
}

/// The pieces that paint a bar filled `fraction` of the way: gpui gradients have only two stops.
pub fn segments(fraction: f32) -> Vec<Segment> {
    let fraction = fraction.clamp(0.0, 1.0);
    GRADIENT_STOPS
        .windows(2)
        .enumerate()
        .filter(|(_, stops)| stops[0] < fraction)
        .map(|(stop, stops)| {
            let to = stops[1].min(fraction);
            Segment {
                stop,
                from: stops[0],
                to,
                reach: (to - stops[0]) / (stops[1] - stops[0]),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn disk() -> DiskUsage {
        DiskUsage {
            total: 500,
            available: 80,
            free: 100,
        }
    }

    #[test]
    fn the_preview_takes_the_selection_off_the_used_bytes() {
        let preview = UsagePreview::new(disk(), 150);
        assert_eq!(preview.used, 400);
        assert_eq!(preview.used_after, 250);
        assert_eq!(preview.free_after, 230);
        assert_eq!(preview.fraction(), 0.8);
        assert_eq!(preview.fraction_after(), 0.5);
    }

    #[test]
    fn a_selection_bigger_than_the_used_bytes_empties_the_disk_at_most() {
        let preview = UsagePreview::new(disk(), 9_000);
        assert_eq!(preview.freed, 400);
        assert_eq!(preview.used_after, 0);
        assert_eq!(preview.free_after, 480);
    }

    #[test]
    fn an_empty_report_of_the_disk_does_not_divide_by_zero() {
        let preview = UsagePreview::new(
            DiskUsage {
                total: 0,
                available: 0,
                free: 0,
            },
            10,
        );
        assert_eq!(preview.fraction(), 0.0);
    }

    #[test]
    fn severity_changes_at_60_and_85_percent() {
        assert_eq!(Severity::of(0.59), Severity::Low);
        assert_eq!(Severity::of(0.60), Severity::Mid);
        assert_eq!(Severity::of(0.84), Severity::Mid);
        assert_eq!(Severity::of(0.85), Severity::High);
    }

    #[test]
    fn the_fill_cuts_the_gradient_at_its_end() {
        let pieces = segments(0.5);
        assert_eq!(pieces.len(), 2);
        assert_eq!(
            (pieces[0].from, pieces[0].to, pieces[0].reach),
            (0.0, 0.35, 1.0)
        );
        assert_eq!(pieces[1].stop, 1);
        assert_eq!(pieces[1].to, 0.5);
        assert!((pieces[1].reach - 0.15 / 0.27).abs() < 1e-5);
    }

    #[test]
    fn empty_and_full_bars() {
        assert!(segments(0.0).is_empty());
        let full = segments(1.0);
        assert_eq!(full.len(), 4);
        assert_eq!(full[3].to, 1.0);
        assert_eq!(full[3].reach, 1.0);
    }
}
