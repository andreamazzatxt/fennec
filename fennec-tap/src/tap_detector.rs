use std::time::{Duration, Instant};

pub struct TapDetector {
    /// Threshold to recognize a slap (in g deviation from gravity)
    threshold: f64,
    /// Cooldown after a slap is detected
    cooldown: Duration,
    /// Timestamp of last detected slap
    last_slap: Instant,
}

impl TapDetector {
    pub fn new(sensitivity: &str) -> Self {
        // Measured: light taps peak at ~0.06g, slaps at 0.3-0.8g
        let threshold = match sensitivity {
            "low" => 0.30,    // only strong slaps
            "high" => 0.12,   // lighter slaps too
            _ => 0.20,        // medium — safe default
        };

        Self {
            threshold,
            cooldown: Duration::from_millis(2000),
            last_slap: Instant::now() - Duration::from_secs(10),
        }
    }

    /// Feed an accelerometer sample. Returns true if a slap is detected.
    pub fn feed(&mut self, x: f64, y: f64, z: f64) -> bool {
        let mag = (x * x + y * y + z * z).sqrt();
        let deviation = (mag - 1.0).abs();

        if deviation > self.threshold && self.last_slap.elapsed() > self.cooldown {
            self.last_slap = Instant::now();
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_no_trigger() {
        let mut d = TapDetector::new("medium");
        assert!(!d.feed(0.0, 0.0, 1.0));
        assert!(!d.feed(0.01, 0.02, 0.99));
    }

    #[test]
    fn light_tap_no_trigger() {
        let mut d = TapDetector::new("medium");
        // 0.06g deviation — typical light tap
        assert!(!d.feed(0.0, 0.0, 1.06));
    }

    #[test]
    fn slap_triggers() {
        let mut d = TapDetector::new("medium");
        // 0.5g deviation — typical slap
        assert!(d.feed(0.0, 0.0, 1.5));
    }

    #[test]
    fn cooldown_blocks_rapid() {
        let mut d = TapDetector::new("medium");
        assert!(d.feed(0.0, 0.0, 1.5));
        // Immediate second slap blocked
        assert!(!d.feed(0.0, 0.0, 1.5));
    }
}
