use daw_transport::PPQN;

#[derive(Debug, Clone, Copy)]
pub struct TimeSignature {
    pub numerator: u32,
    pub denominator: u32,
}

impl TimeSignature {
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn beats_per_bar(&self) -> u32 {
        self.numerator
    }

    pub fn ticks_per_bar(&self) -> u64 {
        PPQN * self.numerator as u64
    }
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self::new(4, 4)
    }
}

impl From<(u32, u32)> for TimeSignature {
    fn from((numerator, denominator): (u32, u32)) -> Self {
        Self::new(numerator, denominator)
    }
}

impl From<TimeSignature> for (u32, u32) {
    fn from(ts: TimeSignature) -> Self {
        (ts.numerator, ts.denominator)
    }
}

/// Musical time context for tempo-aware conversions.
///
/// `TimeContext` handles all conversions between musical time units (ticks, beats, bars)
/// and physical time units (seconds, samples). It is intentionally free of any UI/pixel
/// concerns - frontends should maintain their own zoom state (pixels_per_beat) and
/// compute pixel positions from beats.
///
/// # Time Units
///
/// - **Ticks**: Smallest unit, PPQN ticks per quarter note (beat)
/// - **Beats**: Quarter notes, tempo-dependent duration
/// - **Bars**: Groups of beats determined by time signature
/// - **Seconds**: Physical time
/// - **Samples**: Audio samples at a given sample rate
#[derive(Debug, Clone, Copy)]
pub struct TimeContext {
    pub tempo: f64,
    pub time_signature: TimeSignature,
}

impl TimeContext {
    pub fn new(tempo: f64, time_signature: impl Into<TimeSignature>) -> Self {
        Self {
            tempo,
            time_signature: time_signature.into(),
        }
    }

    pub fn ticks_to_beats(&self, ticks: u64) -> f64 {
        ticks as f64 / PPQN as f64
    }

    pub fn beats_to_ticks(&self, beats: f64) -> u64 {
        (beats * PPQN as f64) as u64
    }

    pub fn ticks_to_bars(&self, ticks: u64) -> f64 {
        let beats = self.ticks_to_beats(ticks);
        beats / self.time_signature.beats_per_bar() as f64
    }

    pub fn bars_to_ticks(&self, bars: f64) -> u64 {
        let beats = bars * self.time_signature.beats_per_bar() as f64;
        self.beats_to_ticks(beats)
    }

    pub fn ticks_to_seconds(&self, ticks: u64) -> f64 {
        let beats = self.ticks_to_beats(ticks);
        beats * 60.0 / self.tempo
    }

    pub fn seconds_to_ticks(&self, seconds: f64) -> u64 {
        let beats = seconds * self.tempo / 60.0;
        self.beats_to_ticks(beats)
    }

    pub fn ticks_to_samples(&self, ticks: u64, sample_rate: u32) -> u64 {
        let seconds = self.ticks_to_seconds(ticks);
        (seconds * sample_rate as f64) as u64
    }

    pub fn samples_to_ticks(&self, samples: u64, sample_rate: u32) -> u64 {
        let seconds = samples as f64 / sample_rate as f64;
        self.seconds_to_ticks(seconds)
    }

    pub fn format_position(&self, ticks: u64) -> MusicalPosition {
        let total_beats = self.ticks_to_beats(ticks);
        let beats_per_bar = self.time_signature.beats_per_bar() as f64;

        let bar = (total_beats / beats_per_bar).floor() as u32 + 1;
        let beat_in_bar = (total_beats % beats_per_bar).floor() as u32 + 1;
        let tick_in_beat = (ticks % PPQN) as u32;

        MusicalPosition {
            bar,
            beat: beat_in_bar,
            tick: tick_in_beat,
        }
    }
}

impl Default for TimeContext {
    fn default() -> Self {
        Self::new(120.0, TimeSignature::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicalPosition {
    pub bar: u32,
    pub beat: u32,
    pub tick: u32,
}

impl std::fmt::Display for MusicalPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{:03}", self.bar, self.beat, self.tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticks_samples_roundtrip() {
        let ctx = TimeContext::new(120.0, (4, 4));
        let sample_rate = 44100;

        // Test various tick values
        for ticks in [0, 1, 960, 1920, 9600, 96000] {
            let samples = ctx.ticks_to_samples(ticks, sample_rate);
            let back = ctx.samples_to_ticks(samples, sample_rate);
            // Allow 1 tick of rounding error due to floating point
            assert!(
                (back as i64 - ticks as i64).abs() <= 1,
                "roundtrip failed: {} -> {} -> {} (diff: {})",
                ticks,
                samples,
                back,
                (back as i64 - ticks as i64).abs()
            );
        }
    }

    #[test]
    fn test_ticks_to_samples_known_values() {
        // At 120 BPM: 1 beat = 0.5 seconds, 1 tick = 0.5/960 seconds
        // At 44100 Hz: 1 beat = 22050 samples
        let ctx = TimeContext::new(120.0, (4, 4));
        let sample_rate = 44100;

        // 1 beat (PPQN ticks) should equal 22050 samples
        let samples = ctx.ticks_to_samples(PPQN, sample_rate);
        assert_eq!(samples, 22050, "1 beat at 120 BPM should be 22050 samples");

        // 4 beats (1 bar in 4/4) should equal 88200 samples
        let samples = ctx.ticks_to_samples(PPQN * 4, sample_rate);
        assert_eq!(samples, 88200, "1 bar at 120 BPM should be 88200 samples");
    }

    #[test]
    fn test_samples_to_ticks_known_values() {
        let ctx = TimeContext::new(120.0, (4, 4));
        let sample_rate = 44100;

        // 22050 samples should equal 1 beat (PPQN ticks) at 120 BPM
        let ticks = ctx.samples_to_ticks(22050, sample_rate);
        assert_eq!(ticks, PPQN, "22050 samples should be 1 beat");

        // 88200 samples should equal 4 beats
        let ticks = ctx.samples_to_ticks(88200, sample_rate);
        assert_eq!(ticks, PPQN * 4, "88200 samples should be 4 beats");
    }

    #[test]
    fn test_tempo_affects_conversion() {
        let sample_rate = 44100;

        // At 60 BPM: 1 beat = 1 second = 44100 samples
        let ctx_60 = TimeContext::new(60.0, (4, 4));
        let samples_60 = ctx_60.ticks_to_samples(PPQN, sample_rate);
        assert_eq!(
            samples_60, 44100,
            "1 beat at 60 BPM should be 44100 samples"
        );

        // At 120 BPM: 1 beat = 0.5 seconds = 22050 samples
        let ctx_120 = TimeContext::new(120.0, (4, 4));
        let samples_120 = ctx_120.ticks_to_samples(PPQN, sample_rate);
        assert_eq!(
            samples_120, 22050,
            "1 beat at 120 BPM should be 22050 samples"
        );

        // Doubling tempo should halve the sample count
        assert_eq!(samples_60, samples_120 * 2);
    }

    #[test]
    fn test_sample_rate_affects_conversion() {
        let ctx = TimeContext::new(120.0, (4, 4));

        // 1 beat at different sample rates
        let samples_44100 = ctx.ticks_to_samples(PPQN, 44100);
        let samples_48000 = ctx.ticks_to_samples(PPQN, 48000);

        assert_eq!(samples_44100, 22050);
        assert_eq!(samples_48000, 24000);
    }

    #[test]
    fn test_zero_ticks() {
        let ctx = TimeContext::new(120.0, (4, 4));
        assert_eq!(ctx.ticks_to_samples(0, 44100), 0);
        assert_eq!(ctx.samples_to_ticks(0, 44100), 0);
    }
}
