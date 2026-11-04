pub fn format_time(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn fraction_at(x: f32, left: f32, width: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    ((x - left) / width).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_mm_ss() {
        assert_eq!(format_time(0), "0:00");
    }

    #[test]
    fn formats_minutes() {
        assert_eq!(format_time(293), "4:53");
    }

    #[test]
    fn formats_hours() {
        assert_eq!(format_time(3723), "1:02:03");
    }

    #[test]
    fn clamps_fraction() {
        assert_eq!(fraction_at(-5.0, 0.0, 200.0), 0.0);
        assert_eq!(fraction_at(100.0, 0.0, 200.0), 0.5);
        assert_eq!(fraction_at(250.0, 0.0, 200.0), 1.0);
        assert_eq!(fraction_at(50.0, 0.0, 0.0), 0.0);
    }
}
