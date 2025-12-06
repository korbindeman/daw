use daw_core::{
    AudioBuffer, Clip, ClipId, PPQN, Project, Session, TimeSignature, Track, TrackId, WaveformData,
    decode_file, strip_samples_root,
};
use eframe::egui;
use std::collections::HashMap;
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
    pages: Vec<[bool; NUM_STEPS]>, // Each page has 16 steps
    audio: Option<Arc<AudioBuffer>>,
    volume: f32, // Linear gain multiplier (0.0 to 1.0)
}

impl Default for SequencerTrackState {
    fn default() -> Self {
        Self {
            sample_path: None,
            sample_name: "Select sample...".to_string(),
            pages: vec![[false; NUM_STEPS]], // Start with one empty page
            audio: None,
            volume: 1.0, // Unity gain by default
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
    current_page: usize,
    playback_page: usize,
    loop_current_page: bool,
    session: Option<Session>,
    project_name: String,
    error_message: Option<String>,
    show_inspector: bool,
    current_project: Option<Project>,
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
            current_page: 0,
            playback_page: 0,
            loop_current_page: false,
            session: None,
            project_name: "Untitled".to_string(),
            error_message: None,
            show_inspector: false,
            current_project: None,
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
                self.tracks[track_idx].sample_path = Some(strip_samples_root(path));
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

                if self.loop_current_page {
                    // Only use the current page
                    if self.current_page < track.pages.len() {
                        let page_steps = &track.pages[self.current_page];
                        for (step_idx, &active) in page_steps.iter().enumerate() {
                            if active {
                                let waveform = WaveformData::from_audio_buffer(audio, 512);
                                let clip_id = (track_idx * NUM_STEPS + step_idx) as u64;
                                clips.push(Clip {
                                    id: ClipId(clip_id),
                                    name: format!("{} {}", track.sample_name, clip_num),
                                    start: (step_idx as u64) * ticks_per_step,
                                    audio: audio.clone(),
                                    waveform: Arc::new(waveform),
                                });
                                clip_num += 1;
                            }
                        }
                    }
                } else {
                    // Iterate through all pages
                    for (page_idx, page_steps) in track.pages.iter().enumerate() {
                        let bar_offset = page_idx as u64 * ticks_per_bar;

                        for (step_idx, &active) in page_steps.iter().enumerate() {
                            if active {
                                let waveform = WaveformData::from_audio_buffer(audio, 512);
                                let clip_id = (track_idx * NUM_STEPS * track.pages.len()
                                    + page_idx * NUM_STEPS
                                    + step_idx)
                                    as u64;
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
                }

                if clips.is_empty() {
                    return None;
                }

                Some(Track {
                    id: TrackId(track_idx as u64),
                    name: track.sample_name.clone(),
                    clips,
                    volume: track.volume,
                })
            })
            .collect()
    }

    fn start_playback(&mut self) {
        // Reuse existing session if available (preserves resample cache)
        if let Some(ref mut session) = self.session {
            session.play();
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
        self.current_step = 0;
        self.playback_page = 0;
    }

    fn update_playback_position(&mut self) {
        if let Some(ref mut session) = self.session {
            if let Some(ticks) = session.poll() {
                let ticks_per_step = Self::ticks_per_step();
                let ticks_per_bar = NUM_STEPS as u64 * ticks_per_step;

                if self.loop_current_page {
                    // Loop the current page only
                    let wrapped_ticks = ticks % ticks_per_bar;
                    self.playback_page = self.current_page;
                    self.current_step = (wrapped_ticks / ticks_per_step) as usize;
                } else {
                    // Play through all pages sequentially
                    let total_pages = self.tracks.iter().map(|t| t.pages.len()).max().unwrap_or(1);
                    let total_ticks = ticks_per_bar * total_pages as u64;
                    let wrapped_ticks = ticks % total_ticks;

                    let calculated_page = (wrapped_ticks / ticks_per_bar) as usize;
                    self.playback_page = calculated_page.min(total_pages.saturating_sub(1));
                    self.current_step = ((wrapped_ticks % ticks_per_bar) / ticks_per_step) as usize;

                    // Auto-scroll to follow playback (clamp to valid range)
                    self.current_page = self.playback_page.min(total_pages.saturating_sub(1));
                }
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

                // Iterate through all pages
                for (page_idx, page_steps) in track.pages.iter().enumerate() {
                    let bar_offset = page_idx as u64 * ticks_per_bar;

                    for (step_idx, &active) in page_steps.iter().enumerate() {
                        if active {
                            let clip_id = (track_idx * NUM_STEPS * track.pages.len()
                                + page_idx * NUM_STEPS
                                + step_idx) as u64;
                            // Session::save() will strip the samples/ prefix
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
                    volume: track.volume,
                })
            })
            .collect();

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAW Project", &["dawproj"])
            .set_file_name(&format!("{}.dawproj", self.project_name))
            .save_file()
        {
            // Create a temporary session just for saving
            match Session::new_with_audio_paths(
                transport_tracks,
                self.tempo,
                self.time_signature,
                audio_paths,
            ) {
                Ok(mut save_session) => {
                    save_session.set_name(self.project_name.clone());
                    match save_session.save(&path) {
                        Ok(()) => {
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Failed to save: {}", e));
                        }
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to create session for save: {}", e));
                }
            }
        }
    }

    fn load_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAW Project", &["dawproj"])
            .set_directory("projects")
            .pick_file()
        {
            match Session::from_project(&path) {
                Ok(loaded_session) => {
                    // Stop and clear the old session
                    if let Some(ref mut session) = self.session {
                        session.stop();
                    }
                    self.session = None;

                    self.project_name = loaded_session.name().to_string();
                    self.tempo = loaded_session.tempo();
                    self.tempo_input = format!("{:.0}", loaded_session.tempo());
                    let time_sig = loaded_session.time_signature();
                    self.time_signature = time_sig;
                    self.time_sig_numerator_input = format!("{}", time_sig.numerator);
                    self.time_sig_denominator_input = format!("{}", time_sig.denominator);
                    self.tracks.clear();

                    let ticks_per_step = Self::ticks_per_step();
                    let ticks_per_bar = NUM_STEPS as u64 * ticks_per_step;

                    for track in loaded_session.tracks() {
                        let mut track_state = SequencerTrackState::default();
                        track_state.pages.clear(); // Clear the default page
                        track_state.volume = track.volume; // Load volume from project

                        // Group clips by page
                        let max_page = track
                            .clips
                            .iter()
                            .map(|clip| (clip.start / ticks_per_bar) as usize)
                            .max()
                            .unwrap_or(0);

                        // Initialize pages
                        for _ in 0..=max_page {
                            track_state.pages.push([false; NUM_STEPS]);
                        }

                        for clip in &track.clips {
                            let page_idx = (clip.start / ticks_per_bar) as usize;
                            let step_idx = ((clip.start % ticks_per_bar) / ticks_per_step) as usize;
                            if page_idx < track_state.pages.len() && step_idx < NUM_STEPS {
                                track_state.pages[page_idx][step_idx] = true;

                                // Get the audio path from the session's audio_paths map
                                if track_state.sample_path.is_none() {
                                    if let Some(audio_path) =
                                        loaded_session.audio_paths().get(&clip.id.0)
                                    {
                                        track_state.sample_path = Some(audio_path.clone());
                                        track_state.sample_name = audio_path
                                            .file_stem()
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "Unknown".to_string());
                                    }
                                    // Use the already-decoded audio from the clip
                                    track_state.audio = Some(clip.audio.clone());
                                }
                            }
                        }

                        self.tracks.push(track_state);
                    }

                    while self.tracks.len() < DEFAULT_TRACKS {
                        self.tracks.push(SequencerTrackState::default());
                    }

                    self.current_page = 0;
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load: {}", e));
                }
            }
        }
    }

    fn build_current_project(&self) -> Project {
        let ticks_per_step = Self::ticks_per_step();
        let ticks_per_bar = NUM_STEPS as u64 * ticks_per_step;

        let tracks: Vec<daw_core::TrackData> = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(track_idx, track)| {
                if track.audio.is_none() {
                    return None;
                }

                let mut clips: Vec<daw_core::ClipData> = Vec::new();
                let mut clip_num = 1;

                for (page_idx, page_steps) in track.pages.iter().enumerate() {
                    let bar_offset = page_idx as u64 * ticks_per_bar;

                    for (step_idx, &active) in page_steps.iter().enumerate() {
                        if active {
                            let clip_id = (track_idx * NUM_STEPS * track.pages.len()
                                + page_idx * NUM_STEPS
                                + step_idx) as u64;

                            clips.push(daw_core::ClipData {
                                id: clip_id,
                                name: format!("{} {}", track.sample_name, clip_num),
                                start: bar_offset + (step_idx as u64) * ticks_per_step,
                                audio_path: track.sample_path.clone().unwrap_or_default(),
                            });
                            clip_num += 1;
                        }
                    }
                }

                Some(daw_core::TrackData {
                    id: track_idx as u64,
                    name: track.sample_name.clone(),
                    clips,
                    volume: track.volume,
                })
            })
            .collect();

        Project {
            name: self.project_name.clone(),
            tempo: self.tempo,
            time_signature: (
                self.time_signature.numerator,
                self.time_signature.denominator,
            ),
            tracks,
        }
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

                ui.add_space(20.0);

                if ui.button("🔍 Inspector").clicked() {
                    self.show_inspector = !self.show_inspector;
                    if self.show_inspector {
                        self.current_project = Some(self.build_current_project());
                    }
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

                // Metronome toggle
                let metronome_enabled = self
                    .session
                    .as_ref()
                    .map_or(false, |s| s.metronome_enabled());
                let metronome_label = if metronome_enabled {
                    "🔔 Metro"
                } else {
                    "🔕 Metro"
                };
                if ui.button(metronome_label).clicked() {
                    if let Some(ref mut session) = self.session {
                        session.toggle_metronome();
                    }
                }

                if ui.button("+ Add Track").clicked() {
                    self.tracks.push(SequencerTrackState::default());
                }
            });

            ui.separator();

            // Page controls
            ui.horizontal(|ui| {
                let max_pages = self.tracks.iter().map(|t| t.pages.len()).max().unwrap_or(1);

                // Previous page button
                if ui.button("◀").clicked() && self.current_page > 0 {
                    self.current_page -= 1;
                }

                ui.label(format!("Page {} / {}", self.current_page + 1, max_pages));

                // Next page button
                if ui.button("▶").clicked() && self.current_page < max_pages - 1 {
                    self.current_page += 1;
                }

                ui.add_space(10.0);

                // Loop current page toggle
                let loop_response = ui.checkbox(&mut self.loop_current_page, "Loop Current Page");
                if loop_response.changed() {
                    // Rebuild tracks when loop mode changes
                    let was_playing = self.session.as_ref().map_or(false, |s| s.is_playing());
                    let tracks = self.build_transport_tracks();

                    if let Some(ref mut session) = self.session {
                        if was_playing {
                            session.stop();
                        }
                        *session.tracks_mut() = tracks;
                        session.update_tracks();

                        if was_playing {
                            session.play();
                        }
                    }
                }

                ui.add_space(10.0);

                // Add empty page
                if ui.button("+ Empty Page").clicked() {
                    for track in &mut self.tracks {
                        track.pages.push([false; NUM_STEPS]);
                    }
                }

                // Duplicate current page
                if ui.button("Duplicate Page").clicked() {
                    for track in &mut self.tracks {
                        if self.current_page < track.pages.len() {
                            let page_to_duplicate = track.pages[self.current_page];
                            track.pages.push(page_to_duplicate);
                        } else {
                            track.pages.push([false; NUM_STEPS]);
                        }
                    }
                }

                // Remove current page
                if max_pages > 1 && ui.button("Remove Page").clicked() {
                    for track in &mut self.tracks {
                        if track.pages.len() > 1 && self.current_page < track.pages.len() {
                            track.pages.remove(self.current_page);
                        }
                    }
                    // Adjust current_page if needed
                    let new_max_pages =
                        self.tracks.iter().map(|t| t.pages.len()).max().unwrap_or(1);
                    if self.current_page >= new_max_pages {
                        self.current_page = new_max_pages.saturating_sub(1);
                    }

                    // Rebuild tracks if playing
                    if self.session.is_some() {
                        let tracks = self.build_transport_tracks();
                        if let Some(ref mut session) = self.session {
                            *session.tracks_mut() = tracks;
                            session.update_tracks();
                        }
                    }
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

                        // Volume slider
                        let volume_response = ui.add(
                            egui::Slider::new(&mut track.volume, 0.0..=1.0)
                                .text("Vol")
                                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                        );
                        if volume_response.changed() {
                            tracks_modified = true;
                        }

                        // Ensure the current page exists for this track
                        while track.pages.len() <= self.current_page {
                            track.pages.push([false; NUM_STEPS]);
                        }

                        // Show steps for the current page
                        let current_page_steps = &mut track.pages[self.current_page];
                        for (step_idx, step) in current_page_steps.iter_mut().enumerate() {
                            // Highlight step if we're playing and on the same page and step
                            let is_current = is_playing
                                && step_idx == self.current_step
                                && self.current_page == self.playback_page;

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

        // Show inspector in separate OS window
        if self.show_inspector {
            let project = self.current_project.clone();

            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("inspector_window"),
                egui::ViewportBuilder::default()
                    .with_title("Project Inspector")
                    .with_inner_size([700.0, 600.0])
                    .with_resizable(true),
                move |ctx, _class| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.heading("Current Project Structure");
                        ui.separator();

                        if let Some(ref proj) = project {
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.monospace(format!("{:#?}", proj));
                                });
                        } else {
                            ui.label("No project data available");
                        }
                    });
                },
            );
        }
    }
}
