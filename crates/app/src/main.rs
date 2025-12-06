mod app_menus;
mod config;
mod keybindings;
mod theme;
mod ui;

use app_menus::{OpenProject, RenderProject, app_menus};
use config::Config;
use daw_core::{ClipId, Session};
use gpui::{
    App, Application, Context, Entity, FocusHandle, Timer, Window, WindowOptions, actions, div,
    prelude::*, px,
};
use keybindings::keybindings;
use std::path::{Path, PathBuf};
use std::time::Duration;
use theme::ActiveTheme;
use ui::{Header, HeaderEvent, Playhead, Sidebar, TimelineRuler, Track, TrackEvent, TrackLabels, TrackLabelsEvent};

struct Daw {
    session: Session,
    header_handle: Entity<Header>,
    playhead_handle: Entity<Playhead>,
    track_labels_handle: Entity<TrackLabels>,
    focus_handle: FocusHandle,
    project_path: PathBuf,
    selected_clips: Vec<ClipId>,
    last_tick: Option<u64>,
    config: Config,
}

impl Daw {
    fn new(cx: &mut Context<Self>) -> Self {
        Self::from_path(Path::new("projects/present_tense.dawproj"), cx)
    }

    fn from_path(path: &Path, cx: &mut Context<Self>) -> Self {
        let session = Session::from_project(path).expect("Failed to load project");

        let time_signature = session.time_signature();
        let tempo = session.tempo();

        let header = cx.new(|cx| {
            Header::new(
                tempo,
                time_signature.numerator,
                time_signature.denominator,
                cx,
            )
        });
        cx.subscribe(
            &header,
            |this, header, event: &HeaderEvent, cx| match event {
                HeaderEvent::Play => this.play(&header, cx),
                HeaderEvent::Pause => this.pause(&header, cx),
                HeaderEvent::Stop => this.stop(&header, cx),
                HeaderEvent::ToggleMetronome => this.toggle_metronome(&header, cx),
            },
        )
        .detach();

        let pixels_per_beat = session.time_context().pixels_per_beat;
        let playhead = cx.new(|_| Playhead::new(0, pixels_per_beat));

        let tracks = session.tracks().to_vec();
        let track_labels = cx.new(|_| TrackLabels::new(tracks));
        cx.subscribe(&track_labels, |this, _entity, event: &TrackLabelsEvent, cx| {
            match event {
                TrackLabelsEvent::ToggleEnabled(track_id) => {
                    this.session.toggle_track_enabled(*track_id);
                    this.update_track_labels(cx);
                    cx.notify();
                }
            }
        })
        .detach();

        let focus_handle = cx.focus_handle();

        Self {
            session,
            header_handle: header,
            playhead_handle: playhead,
            track_labels_handle: track_labels,
            focus_handle,
            project_path: path.to_path_buf(),
            selected_clips: Vec::new(),
            last_tick: None,
            config: Config::load(),
        }
    }

    fn load_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // If same project, do nothing
        if path == self.project_path {
            return;
        }

        // Load new session
        match Session::from_project(&path) {
            Ok(session) => {
                // Get new project settings
                let time_signature = session.time_signature();
                let tempo = session.tempo();

                // Update session and project state
                self.session = session;
                self.project_path = path;
                self.selected_clips.clear();

                // Update header with new values
                self.header_handle.update(cx, |header, cx| {
                    header.set_tick(0, cx);
                    header.set_playing(false, cx);
                    header.update_values(
                        tempo,
                        time_signature.numerator,
                        time_signature.denominator,
                        cx,
                    );
                });

                self.playhead_handle.update(cx, |playhead, cx| {
                    playhead.set_tick(0);
                    cx.notify();
                });

                // Update track labels with new tracks
                self.update_track_labels(cx);

                cx.notify();
            }
            Err(e) => {
                eprintln!("Failed to load project: {}", e);
            }
        }
    }

    fn toggle_clip_selection(&mut self, clip_id: ClipId, cx: &mut Context<Self>) {
        self.selected_clips.clear();
        self.selected_clips.push(clip_id.clone());
        cx.notify();
    }

    fn update_track_labels(&mut self, cx: &mut Context<Self>) {
        let tracks = self.session.tracks().to_vec();
        self.track_labels_handle.update(cx, |track_labels, cx| {
            track_labels.set_tracks(tracks);
            cx.notify();
        });
    }

    fn poll_status(&mut self, cx: &mut Context<Self>) {
        if let Some(tick) = self.session.poll() {
            // Only update UI if tick actually changed
            if self.last_tick != Some(tick) {
                self.last_tick = Some(tick);

                // Batch updates: update entities without individual notifications
                self.header_handle.update(cx, |header, cx| {
                    header.set_tick_silent(tick, cx);
                });
                self.playhead_handle.update(cx, |playhead, _cx| {
                    playhead.set_tick(tick);
                });

                // Single notification for all updates
                cx.notify();
            }
        }
    }

    fn play(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.play();
        header.update(cx, |header, cx| header.set_playing(true, cx));

        // Start polling loop when playback starts
        cx.spawn(
            async |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                loop {
                    Timer::after(Duration::from_millis(16)).await;

                    let should_continue = cx.update(|cx| {
                        this.update(cx, |daw, cx| {
                            daw.poll_status(cx);
                            daw.session.is_playing()
                        })
                    });

                    match should_continue {
                        Ok(Ok(true)) => continue,
                        _ => break,
                    }
                }
            },
        )
        .detach();
    }

    fn pause(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.pause();
        header.update(cx, |header, cx| header.set_playing(false, cx));
    }

    fn stop(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.stop();
        self.last_tick = None;
        header.update(cx, |header, cx| header.set_playing(false, cx));
    }

    fn toggle_metronome(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.toggle_metronome();
        let enabled = self.session.metronome_enabled();
        header.update(cx, |header, cx| header.set_metronome_enabled(enabled, cx));
    }
}

impl Render for Daw {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        let timeline_width = self.session.calculate_timeline_width();
        let time_ctx = self.session.time_context();
        let pixels_per_beat = time_ctx.pixels_per_beat;
        let time_signature = time_ctx.time_signature;
        let tracks = self.session.tracks();

        let header_handle = self.header_handle.clone();

        div()
            .id("root")
            .size_full()
            .bg(theme.background)
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(move |this, _: &PlayPause, _, cx| {
                let is_playing = this.session.is_playing();
                header_handle.update(cx, |_, cx| {
                    if is_playing {
                        cx.emit(HeaderEvent::Stop);
                    } else {
                        cx.emit(HeaderEvent::Play);
                    }
                });
            }))
            .on_action(cx.listener(|this, _: &OpenProject, _, cx| {
                let start_dir = this.config.picker_directories.get("open_project").cloned();
                cx.spawn(
                    async |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .add_filter("DAW Project", &["dawproj"])
                            .set_title("Open Project");
                        if let Some(dir) = start_dir {
                            dialog = dialog.set_directory(&dir);
                        }
                        let file = dialog.pick_file().await;

                        if let Some(file) = file {
                            let path = file.path().to_path_buf();
                            let _ = cx.update(|cx| {
                                this.update(cx, |daw, cx| {
                                    if let Some(parent) = path.parent() {
                                        daw.config.picker_directories.insert(
                                            "open_project".to_string(),
                                            parent.to_path_buf(),
                                        );
                                        daw.config.save();
                                    }
                                    daw.load_project(path, cx);
                                })
                            });
                        }
                    },
                )
                .detach();
            }))
            .on_action(cx.listener(|this, _: &RenderProject, _, cx| {
                let session = &this.session;
                let tempo = session.tempo();
                let tracks = session.tracks().to_vec();
                let start_dir = this.config.picker_directories.get("render").cloned();
                let default_name = this
                    .project_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| format!("{}.wav", s))
                    .unwrap_or_else(|| "render.wav".to_string());

                cx.spawn(
                    async move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .add_filter("WAV Audio", &["wav"])
                            .set_title("Render to WAV")
                            .set_file_name(&default_name);
                        if let Some(dir) = start_dir {
                            dialog = dialog.set_directory(&dir);
                        }
                        let file = dialog.save_file().await;

                        if let Some(file) = file {
                            let path = file.path().to_path_buf();
                            let _ = cx.update(|cx| {
                                this.update(cx, |daw, _cx| {
                                    if let Some(parent) = path.parent() {
                                        daw.config
                                            .picker_directories
                                            .insert("render".to_string(), parent.to_path_buf());
                                        daw.config.save();
                                    }
                                })
                            });
                            let buffer = daw_core::render_timeline(&tracks, tempo, 44100, 2);
                            if let Err(e) = daw_core::write_wav(&buffer, &path) {
                                eprintln!("Failed to render: {}", e);
                            }
                        }
                    },
                )
                .detach();
            }))
            .child(self.header_handle.clone())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(cx.new(|_| Sidebar::new()))
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .bg(theme.elevated)
                            // .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right(px(150.))
                                    .bottom_0()
                                    .flex()
                                    .flex_col()
                                    .child(cx.new(|_| {
                                        TimelineRuler::new(
                                            pixels_per_beat,
                                            time_signature.into(),
                                            timeline_width,
                                        )
                                    }))
                                    .child({
                                        let selected_clips = self.selected_clips.clone();
                                        let tracks_and_playhead = div()
                                            .flex_1()
                                            .relative()
                                            .children(tracks.iter().map(|track| {
                                                let selected_clips = selected_clips.clone();
                                                let track_entity = cx.new(|_| {
                                                    Track::new(
                                                        track.clone(),
                                                        pixels_per_beat,
                                                        self.session.tempo(),
                                                        timeline_width,
                                                    )
                                                    .selected_clips(selected_clips)
                                                });

                                                cx.subscribe(
                                                    &track_entity,
                                                    |this, _track, event: &TrackEvent, cx| {
                                                        match event {
                                                            TrackEvent::ClipClicked(clip_id) => {
                                                                this.toggle_clip_selection(
                                                                    clip_id.clone(),
                                                                    cx,
                                                                );
                                                            }
                                                        }
                                                    },
                                                )
                                                .detach();

                                                track_entity
                                            }));

                                        tracks_and_playhead.child(self.playhead_handle.clone())
                                    }),
                            )
                            .child(self.track_labels_handle.clone()),
                    ),
            )
    }
}

actions!(daw, [PlayPause, Quit]);

fn main() {
    Application::new().run(|cx: &mut App| {
        theme::init(cx);

        // Set up menus
        cx.set_menus(app_menus());

        // Set up actions
        cx.on_action(|_: &Quit, cx: &mut App| {
            cx.quit();
        });

        // Bind keys
        cx.bind_keys(keybindings());
        ui::primitives::input::bind_input_keys(cx);

        // Open window
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|cx| Daw::new(cx)))
            .unwrap();
    });
}
