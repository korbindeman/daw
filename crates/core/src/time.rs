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

#[derive(Debug, Clone, Copy)]
pub struct TimeContext {
    pub tempo: f64,
    pub time_signature: TimeSignature,
    pub pixels_per_beat: f64,
}

impl TimeContext {
    pub fn new(tempo: f64, time_signature: impl Into<TimeSignature>, pixels_per_beat: f64) -> Self {
        Self {
            tempo,
            time_signature: time_signature.into(),
            pixels_per_beat,
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

    pub fn ticks_to_pixels(&self, ticks: u64) -> f64 {
        self.ticks_to_beats(ticks) * self.pixels_per_beat
    }

    pub fn pixels_to_ticks(&self, pixels: f64) -> u64 {
        let beats = pixels / self.pixels_per_beat;
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
        Self::new(120.0, TimeSignature::default(), 100.0)
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
