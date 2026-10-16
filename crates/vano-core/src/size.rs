//! Byte counts in decimal units (1 GB = 10^9 bytes), the units disk makers and file managers
//! use.

/// A unit for a byte count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    B,
    Kb,
    Mb,
    Gb,
    Tb,
}

impl Unit {
    /// The unit symbol (`GB`, ...).
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::B => "B",
            Unit::Kb => "KB",
            Unit::Mb => "MB",
            Unit::Gb => "GB",
            Unit::Tb => "TB",
        }
    }
}

/// Splits `bytes` into a value and the largest unit that keeps the value at 1 or more, rounded
/// the way the design shows sizes: no decimals at 100 or more, one decimal below (bytes are
/// always whole).
pub fn split(bytes: u64) -> (f64, Unit, usize) {
    const STEPS: [(f64, Unit); 4] = [
        (1e12, Unit::Tb),
        (1e9, Unit::Gb),
        (1e6, Unit::Mb),
        (1e3, Unit::Kb),
    ];
    let b = bytes as f64;
    for (scale, unit) in STEPS {
        if b >= scale * 0.9995 {
            let value = b / scale;
            let decimals = if value >= 99.95 { 0 } else { 1 };
            return (value, unit, decimals);
        }
    }
    (b, Unit::B, 0)
}

/// `bytes` as plain text with a `.` decimal separator, for logs and tests (`13.4 GB`). The UI
/// formats the number with the user's locale instead.
pub fn format_plain(bytes: u64) -> String {
    let (value, unit, decimals) = split(bytes);
    format!("{value:.decimals$} {}", unit.symbol())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_units() {
        assert_eq!(format_plain(0), "0 B");
        assert_eq!(format_plain(999), "999 B");
        assert_eq!(format_plain(1_000), "1.0 KB");
        assert_eq!(format_plain(13_400_000_000), "13.4 GB");
        assert_eq!(format_plain(135_000_000_000), "135 GB");
        assert_eq!(format_plain(999_999_999), "1.0 GB");
        assert_eq!(format_plain(2_000_000_000_000), "2.0 TB");
    }
}
