use daw_core::{
    AudioBuffer, Clip, ClipId, PPQN, Project, ProjectError, Session, TimeSignature, Track, TrackId,
    WaveformData, decode_file, save_project,
};
use eframe::egui;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;

const SAMPLES_DIR: &str = "samples";
const NUM_STEPS: usize = 16;
const DEFAULT_TRACKS: usize = 4;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Step Sequencer",
        options,
        Box::new(|_cc| Ok(Box::new(SequencerApp::new()))),
    )
}

struct SequencerTrackState {
    sample_path: Option<PathBuf>,
    sample_name: String,
    steps: [bool; NUM_STEPS],
    audio: Option<Arc<AudioBuffer>>,
}

impl Default for SequencerTrackState {
    fn default() -> Self {
        Self {
            sample_path: None,
            sample_name: "Select sample...".to_string(),
            steps: [false; NUM_STEPS],
            audio: None,
        }
    }
}

struct SequencerApp {
    tracks: Vec<SequencerTrackState>,
    available_samples: Vec<PathBuf>,
    tempo: f64,
    tempo_input: String,
    time_signature: TimeSignature,
    time_sig_numerator_input: String,
    time_sig_denominator_input: String,
    current_step: usize,
    session: Option<Session>,
    project_name: String,
    error_message: Option<String>,
    loop_count: usize,
    loop_count_input: String,
}

impl SequencerApp {
    fn new() -> Self {
        let available_samples = Self::scan_samples();

        let mut tracks = Vec::with_capacity(DEFAULT_TRACKS);
        for _ in 0..DEFAULT_TRACKS {
            tracks.push(SequencerTrackState::default());
        }

        Self {
            tracks,
            available_samples,
            tempo: 120.0,
            tempo_input: "120".to_string(),
            time_signature: TimeSignature::new(4, 4),
            time_sig_numerator_input: "4".to_string(),
            time_sig_denominator_input: "4".to_string(),
            current_step: 0,
            session: None,
            project_name: "Untitled".to_string(),
            error_message: None,
            loop_count: 8,
            loop_count_input: "8".to_string(),
        }
    }

    fn scan_samples() -> Vec<PathBuf> {
        fn scan_dir(dir: &str, samples: &mut Vec<PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(path_str) = path.to_str() {
                            scan_dir(path_str, samples);
                        }
                    } else if path.extension().is_some_and(|e| e == "wav") {
                        samples.push(path);
                    }
                }
            }
        }

        let mut samples = Vec::new();
        scan_dir(SAMPLES_DIR, &mut samples);
        samples.sort();
        samples
    }

    fn load_sample(&mut self, track_idx: usize, path: &PathBuf) {
        match decode_file(path) {
            Ok(buffer) => {
                self.tracks[track_idx].audio = Some(Arc::new(buffer));
                self.tracks[track_idx].sample_path = Some(path.clone());
                self.tracks[track_idx].sample_name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load sample: {}", e));
            }
        }
    }

    fn ticks_per_step() -> u64 {
        PPQN / 4
    }

    fn build_transport_tracks(&self) -> Vec<Track> {
        let ticks_per_step = Self::ticks_per_step();
        let ticks_per_bar = NUM_STEPS as u64 * ticks_per_step;

        self.tracks
            .iter()
            .enumerate()
            .filter_map(|(track_idx, track)| {
                let audio = track.audio.as_ref()?;

                let mut clips: Vec<Clip> = Vec::new();
                let mut clip_num = 1;

                // Duplicate the 16-step pattern loop_count times
                for loop_idx in 0..self.loop_count {
                    let bar_offset = loop_idx as u64 * ticks_per_bar;

                    for (step_idx, &active) in track.steps.iter().enumerate() {
                        if active {
                            let waveform = WaveformData::from_audio_buffer(audio, 512);
                            let clip_id = (track_idx * NUM_STEPS * self.loop_count
                                + loop_idx * NUM_STEPS
                                + step_idx) as u64;
                            clips.push(Clip {
                                id: ClipId(clip_id),
                                name: format!("{} {}", track.sample_name, clip_num),
                                start: bar_offset + (step_idx as u64) * ticks_per_step,
                                audio: audio.clone(),
                                waveform: Arc::new(waveform),
                            });
                            clip_num += 1;
                        }
                    }
                }

                if clips.is_empty() {
                    return None;
                }

                Some(Track {
                    id: TrackId(track_idx as u64),
                    name: track.sample_name.clone(),
                    clips,
                })
            })
            .collect()
    }

    fn start_playback(&mut self) {
        if self.session.is_some() {
            return;
        }

        let tracks = self.build_transport_tracks();
        match Session::new(tracks, self.tempo, self.time_signature) {
            Ok(mut session) => {
                session.play();
                self.session = Some(session);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to start playback: {}", e));
            }
        }
    }

    fn stop_playback(&mut self) {
        if let Some(ref mut session) = self.session {
            session.stop();
        }
        self.session = None;
        self.current_step = 0;
    }

    fn update_playback_position(&mut self) {
        if let Some(ref mut session) = self.session {
            if let Some(ticks) = session.poll() {
                let ticks_per_step = Self::ticks_per_step();
                let ticks_per_bar = NUM_STEPS as u64 * ticks_per_step;
                let total_ticks = ticks_per_bar * self.loop_count as u64;
                let wrapped_ticks = ticks % total_ticks;
                self.current_step = ((wrapped_ticks % ticks_per_bar) / ticks_per_step) as usize;
            }
        }
    }

    fn save_project(&mut self) {
        let ticks_per_step = Self::ticks_per_step();
        let ticks_per_bar = NUM_STEPS as u64 * ticks_per_step;

        let mut audio_paths: HashMap<u64, PathBuf> = HashMap::new();
        let transport_tracks: Vec<Track> = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(track_idx, track)| {
                let audio = track.audio.as_ref()?;
                let sample_path = track.sample_path.as_ref()?;

                let mut clips: Vec<Clip> = Vec::new();
                let mut clip_num = 1;

                // Duplicate the 16-step pattern loop_count times
                for loop_idx in 0..self.loop_count {
                    let bar_offset = loop_idx as u64 * ticks_per_bar;

                    for (step_idx, &active) in track.steps.iter().enumerate() {
                        if active {
                            let clip_id = (track_idx * NUM_STEPS * self.loop_count
                                + loop_idx * NUM_STEPS
                                + step_idx) as u64;
                            audio_paths.insert(clip_id, sample_path.clone());

                            let waveform = WaveformData::from_audio_buffer(audio, 512);
                            clips.push(Clip {
                                id: ClipId(clip_id),
                                name: format!("{} {}", track.sample_name, clip_num),
                                start: bar_offset + (step_idx as u64) * ticks_per_step,
                                audio: audio.clone(),
                                waveform: Arc::new(waveform),
                            });
                            clip_num += 1;
                        }
                    }
                }

                Some(Track {
                    id: TrackId(track_idx as u64),
                    name: track.sample_name.clone(),
                    clips,
                })
            })
            .collect();

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAW Project", &["dawproj"])
            .set_file_name(&format!("{}.dawproj", self.project_name))
            .save_file()
        {
            match save_project(
                &path,
                self.project_name.clone(),
                self.tempo,
                (4, 4),
                &transport_tracks,
                &audio_paths,
            ) {
                Ok(()) => {
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to save: {}", e));
                }
            }
        }
    }

    fn load_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAW Project", &["dawproj"])
            .pick_file()
        {
            match Self::load_project_file(&path) {
                Ok(project) => {
                    self.project_name = project.name;
                    self.tempo = project.tempo;
                    self.tracks.clear();

                    let ticks_per_step = Self::ticks_per_step();

                    for track_data in &project.tracks {
                        let mut track_state = SequencerTrackState::default();

                        for clip in &track_data.clips {
                            let step_idx = (clip.start / ticks_per_step) as usize;
                            if step_idx < NUM_STEPS {
                                track_state.steps[step_idx] = true;

                                if track_state.sample_path.is_none()
                                    && !clip.audio_path.as_os_str().is_empty()
                                {
                                    track_state.sample_path = Some(clip.audio_path.clone());
                                    track_state.sample_name = clip
                                        .audio_path
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    if let Ok(buffer) = decode_file(&clip.audio_path) {
                                        track_state.audio = Some(Arc::new(buffer));
                                    }
                                }
                            }
                        }

                        self.tracks.push(track_state);
                    }

                    while self.tracks.len() < DEFAULT_TRACKS {
                        self.tracks.push(SequencerTrackState::default());
                    }

                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load: {}", e));
                }
            }
        }
    }

    fn load_project_file(path: &std::path::Path) -> Result<Project, ProjectError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let project: Project = rmp_serde::decode::from_read(reader)?;
        Ok(project)
    }
}

impl eframe::App for SequencerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_playback_position();

        let is_playing = self.session.as_ref().map_or(false, |s| s.is_playing());
        if is_playing {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Step Sequencer");

            if let Some(error) = &self.error_message {
                ui.colored_label(egui::Color32::RED, error);
            }

            ui.horizontal(|ui| {
                ui.label("Project:");
                ui.text_edit_singleline(&mut self.project_name);

                if ui.button("Save").clicked() {
                    self.save_project();
                }
                if ui.button("Load").clicked() {
                    self.load_project();
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("BPM:");
                let response = ui.text_edit_singleline(&mut self.tempo_input);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(t) = self.tempo_input.parse::<f64>() {
                        if (30.0..=300.0).contains(&t) {
                            self.tempo = t;
                            if let Some(ref mut session) = self.session {
                                session.time_context_mut().tempo = t;
                                session.update_tempo();
                            }
                        } else {
                            self.tempo_input = format!("{:.0}", self.tempo);
                        }
                    } else {
                        self.tempo_input = format!("{:.0}", self.tempo);
                    }
                }

                ui.add_space(20.0);

                ui.label("Loops:");
                let response = ui.text_edit_singleline(&mut self.loop_count_input);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(l) = self.loop_count_input.parse::<usize>() {
                        if (1..=16).contains(&l) {
                            self.loop_count = l;
                        } else {
                            self.loop_count_input = format!("{}", self.loop_count);
                        }
                    } else {
                        self.loop_count_input = format!("{}", self.loop_count);
                    }
                }

                ui.add_space(20.0);

                ui.label("Time Sig:");
                ui.label(&self.time_sig_numerator_input);
                ui.label("/");
                ui.label(&self.time_sig_denominator_input);

                ui.add_space(20.0);

                if is_playing {
                    if ui.button("⏹ Stop").clicked() {
                        self.stop_playback();
                    }
                } else if ui.button("▶ Play").clicked() {
                    self.start_playback();
                }

                if ui.button("+ Add Track").clicked() {
                    self.tracks.push(SequencerTrackState::default());
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.add_space(150.0);
                for i in 0..NUM_STEPS {
                    let label = format!("{}", i + 1);
                    let is_beat = i % self.time_signature.denominator as usize == 0;

                    let (text_color, bg_color) = if is_playing && i == self.current_step {
                        (egui::Color32::BLACK, egui::Color32::YELLOW)
                    } else if is_beat {
                        (egui::Color32::WHITE, egui::Color32::from_rgb(70, 130, 180))
                    } else {
                        (
                            egui::Color32::LIGHT_GRAY,
                            egui::Color32::from_rgb(40, 40, 40),
                        )
                    };

                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(30.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, bg_color);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::default(),
                        text_color,
                    );
                }
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let samples = self.available_samples.clone();
                let mut load_requests: Vec<(usize, PathBuf)> = Vec::new();
                let mut tracks_modified = false;

                for (track_idx, track) in self.tracks.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(format!("sample_{}", track_idx))
                            .selected_text(&track.sample_name)
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                for sample_path in &samples {
                                    let name = sample_path
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    if ui.selectable_label(false, &name).clicked() {
                                        load_requests.push((track_idx, sample_path.clone()));
                                    }
                                }
                            });

                        for (step_idx, step) in track.steps.iter_mut().enumerate() {
                            let is_current = is_playing && step_idx == self.current_step;
                            let color = if *step {
                                if is_current {
                                    egui::Color32::YELLOW
                                } else {
                                    egui::Color32::from_rgb(100, 180, 100)
                                }
                            } else if is_current {
                                egui::Color32::from_rgb(80, 80, 40)
                            } else {
                                egui::Color32::from_rgb(60, 60, 60)
                            };

                            let response =
                                ui.add_sized([30.0, 30.0], egui::Button::new("").fill(color));
                            if response.clicked() {
                                *step = !*step;
                                tracks_modified = true;
                            }
                        }
                    });
                }

                for (track_idx, path) in load_requests {
                    self.load_sample(track_idx, &path);
                }

                if tracks_modified {
                    if self.session.is_some() {
                        let tracks = self.build_transport_tracks();
                        if let Some(ref mut session) = self.session {
                            *session.tracks_mut() = tracks;
                            session.update_tracks();
                        }
                    }
                }
            });
        });
    }
}
