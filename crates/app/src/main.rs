mod app_menus;
mod config;
mod keybindings;
mod theme;
mod ui;

use app_menus::{OpenProject, RenderProject, SaveProject, SaveProjectAs, app_menus};
use config::Config;
use daw_core::Session;
use gpui::{
    App, Application, Context, Entity, FocusHandle, Timer, Window, WindowOptions, actions, div,
    prelude::*, px,
};
use keybindings::keybindings;
use std::path::{Path, PathBuf};
use std::time::Duration;
use theme::ActiveTheme;
use ui::{
    ClipId, Cursor, Header, HeaderEvent, Playhead, RulerEvent, TimelineRuler, Track, TrackEvent,
    TrackLabels, TrackLabelsEvent,
};

// UI Layout Constants
const TRACK_LABEL_WIDTH: f32 = 150.0;

struct Daw {
    session: Session,
    header_handle: Entity<Header>,
    playhead_handle: Entity<Playhead>,
    cursor_handle: Entity<Cursor>,
    track_labels_handle: Entity<TrackLabels>,
    track_entities: Vec<Entity<Track>>,
    focus_handle: FocusHandle,
    project_path: PathBuf,
    selected_clips: Vec<ClipId>,
    last_tick: Option<u64>,
    config: Config,
    scroll_handle: gpui::ScrollHandle,
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
        let cursor = cx.new(|_| Cursor::new(Some(0), pixels_per_beat)); // Initialize at tick 0

        let tracks = session.tracks().to_vec();
        let track_labels = cx.new(|_| TrackLabels::new(tracks.clone()));
        cx.subscribe(
            &track_labels,
            |this, _entity, event: &TrackLabelsEvent, cx| match event {
                TrackLabelsEvent::ToggleEnabled(track_id) => {
                    this.session.toggle_track_enabled(*track_id);
                    this.update_track_labels(cx);
                    cx.notify();
                }
            },
        )
        .detach();

        // Create track entities
        let timeline_width = session.calculate_timeline_width();
        let track_entities: Vec<_> = tracks
            .iter()
            .map(|track| {
                let track_entity =
                    cx.new(|_| Track::new(track.clone(), pixels_per_beat, tempo, timeline_width));
                track_entity
            })
            .collect();

        // Subscribe to track events
        for track_entity in &track_entities {
            cx.subscribe(
                track_entity,
                |this, _track, event: &TrackEvent, cx| match event {
                    TrackEvent::ClipClicked(clip_id) => {
                        this.toggle_clip_selection(clip_id.clone(), cx);
                    }
                    TrackEvent::EmptySpaceClicked(x_pos) => {
                        this.handle_timeline_click(*x_pos, cx);
                    }
                },
            )
            .detach();
        }

        let focus_handle = cx.focus_handle();

        Self {
            session,
            header_handle: header,
            playhead_handle: playhead,
            cursor_handle: cursor,
            track_labels_handle: track_labels,
            track_entities,
            focus_handle,
            project_path: path.to_path_buf(),
            selected_clips: Vec::new(),
            last_tick: None,
            config: Config::load(),
            scroll_handle: gpui::ScrollHandle::new(),
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

                // Reset cursor to beginning
                self.cursor_handle.update(cx, |cursor, cx| {
                    cursor.set_tick(Some(0));
                    cx.notify();
                });

                // Update track labels with new tracks
                self.update_track_labels(cx);

                // Recreate track entities for new project
                self.recreate_track_entities(cx);

                cx.notify();
            }
            Err(e) => {
                eprintln!("Failed to load project: {}", e);
            }
        }
    }

    fn render_grid_lines(
        &self,
        pixels_per_beat: f64,
        time_signature: daw_core::TimeSignature,
        timeline_width: f64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use daw_core::{PPQN, SnapMode};

        let theme = cx.theme();
        let snap_mode = self.session.snap_mode();
        let beats_per_bar = time_signature.numerator;

        let mut grid_lines = vec![];

        match snap_mode {
            SnapMode::None => {
                // No grid lines when snapping is disabled
            }
            SnapMode::Beat => {
                // One line per beat
                let total_beats = (timeline_width / pixels_per_beat).ceil() as u32;
                for beat in 0..=total_beats {
                    let x_pos = beat as f64 * pixels_per_beat;
                    let is_bar_start = beat % beats_per_bar == 0;

                    grid_lines.push(
                        div()
                            .absolute()
                            .left(px(x_pos as f32))
                            .top(px(0.))
                            .bottom(px(0.))
                            .w(px(1.))
                            .bg(if is_bar_start {
                                theme.border
                            } else {
                                theme.border.opacity(0.3)
                            }),
                    );
                }
            }
            SnapMode::HalfBeat => {
                // Two lines per beat
                let total_half_beats = (timeline_width / (pixels_per_beat / 2.0)).ceil() as u32;
                for half_beat in 0..=total_half_beats {
                    let x_pos = half_beat as f64 * (pixels_per_beat / 2.0);
                    let is_beat = half_beat % 2 == 0;
                    let is_bar_start = is_beat && (half_beat / 2) % beats_per_bar == 0;

                    grid_lines.push(
                        div()
                            .absolute()
                            .left(px(x_pos as f32))
                            .top(px(0.))
                            .bottom(px(0.))
                            .w(px(1.))
                            .bg(if is_bar_start {
                                theme.border
                            } else if is_beat {
                                theme.border.opacity(0.5)
                            } else {
                                theme.border.opacity(0.2)
                            }),
                    );
                }
            }
            SnapMode::QuarterBeat => {
                // Four lines per beat
                let total_quarter_beats = (timeline_width / (pixels_per_beat / 4.0)).ceil() as u32;
                for quarter_beat in 0..=total_quarter_beats {
                    let x_pos = quarter_beat as f64 * (pixels_per_beat / 4.0);
                    let is_beat = quarter_beat % 4 == 0;
                    let is_bar_start = is_beat && (quarter_beat / 4) % beats_per_bar == 0;

                    grid_lines.push(
                        div()
                            .absolute()
                            .left(px(x_pos as f32))
                            .top(px(0.))
                            .bottom(px(0.))
                            .w(px(1.))
                            .bg(if is_bar_start {
                                theme.border
                            } else if is_beat {
                                theme.border.opacity(0.5)
                            } else {
                                theme.border.opacity(0.15)
                            }),
                    );
                }
            }
            SnapMode::Bar => {
                // One line per bar
                let ticks_per_bar = time_signature.ticks_per_bar();
                let pixels_per_bar = (ticks_per_bar as f64 / PPQN as f64) * pixels_per_beat;
                let total_bars = (timeline_width / pixels_per_bar).ceil() as u32;

                for bar in 0..=total_bars {
                    let x_pos = bar as f64 * pixels_per_bar;

                    grid_lines.push(
                        div()
                            .absolute()
                            .left(px(x_pos as f32))
                            .top(px(0.))
                            .bottom(px(0.))
                            .w(px(1.))
                            .bg(theme.border),
                    );
                }
            }
        }

        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .children(grid_lines)
    }

    fn handle_timeline_click(&mut self, x_pos: f64, cx: &mut Context<Self>) {
        // x_pos is viewport-relative, need to subtract scroll offset to get content position
        // (scroll_x is negative when scrolled right)
        let scroll_offset = self.scroll_handle.offset();
        let scroll_x: f32 = scroll_offset.x.into();
        let content_x = x_pos - scroll_x as f64;

        // Convert pixel position to ticks
        let tick = self.session.time_context().pixels_to_ticks(content_x);

        // Set cursor in session (will apply snapping)
        self.session.set_cursor(tick);

        // Update cursor UI
        self.cursor_handle.update(cx, |cursor, cx| {
            cursor.set_tick(self.session.cursor_tick());
            cx.notify();
        });
    }

    fn toggle_clip_selection(&mut self, clip_id: ClipId, cx: &mut Context<Self>) {
        self.selected_clips.clear();
        self.selected_clips.push(clip_id.clone());
        self.update_track_selected_clips(cx);
        cx.notify();
    }

    fn update_track_labels(&mut self, cx: &mut Context<Self>) {
        let tracks = self.session.tracks().to_vec();
        self.track_labels_handle.update(cx, |track_labels, cx| {
            track_labels.set_tracks(tracks);
            cx.notify();
        });
    }

    fn recreate_track_entities(&mut self, cx: &mut Context<Self>) {
        let tracks = self.session.tracks().to_vec();
        let pixels_per_beat = self.session.time_context().pixels_per_beat;
        let tempo = self.session.tempo();
        let timeline_width = self.session.calculate_timeline_width();

        // Clear old track entities
        self.track_entities.clear();

        // Create new track entities
        let track_entities: Vec<_> = tracks
            .iter()
            .map(|track| {
                cx.new(|_| Track::new(track.clone(), pixels_per_beat, tempo, timeline_width))
            })
            .collect();

        // Subscribe to track events
        for track_entity in &track_entities {
            cx.subscribe(
                track_entity,
                |this, _track, event: &TrackEvent, cx| match event {
                    TrackEvent::ClipClicked(clip_id) => {
                        this.toggle_clip_selection(clip_id.clone(), cx);
                    }
                    TrackEvent::EmptySpaceClicked(x_pos) => {
                        this.handle_timeline_click(*x_pos, cx);
                    }
                },
            )
            .detach();
        }

        self.track_entities = track_entities;
    }

    fn update_track_selected_clips(&mut self, cx: &mut Context<Self>) {
        let selected_clips = self.selected_clips.clone();
        for track_entity in &self.track_entities {
            track_entity.update(cx, |track, cx| {
                track.set_selected_clips(selected_clips.clone());
                cx.notify();
            });
        }
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
        let was_playing = self.session.is_playing();
        self.session.stop();

        if was_playing {
            // Just paused - update header only
            header.update(cx, |header, cx| header.set_playing(false, cx));
        } else {
            // Reset to beginning - update cursor and playhead UI
            self.last_tick = None;
            header.update(cx, |header, cx| {
                header.set_tick(0, cx);
                header.set_playing(false, cx);
            });
            self.playhead_handle.update(cx, |playhead, cx| {
                playhead.set_tick(0);
                cx.notify();
            });
            self.cursor_handle.update(cx, |cursor, cx| {
                cursor.set_tick(Some(0));
                cx.notify();
            });
        }
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

        let header_handle = self.header_handle.clone();

        // Create ruler (without click handler - ruler shouldn't move cursor)
        let ruler =
            cx.new(|_| TimelineRuler::new(pixels_per_beat, time_signature.into(), timeline_width));

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
                        // Stop playback (pauses, next press will play from cursor)
                        cx.emit(HeaderEvent::Stop);
                    } else {
                        // Play from cursor (or resume from pause if paused)
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
            .on_action(cx.listener(|this, _: &SaveProject, _, _cx| {
                if let Err(e) = this.session.save_in_place() {
                    eprintln!("Failed to save project: {}", e);
                }
            }))
            .on_action(cx.listener(|this, _: &SaveProjectAs, _, cx| {
                let start_dir = this.config.picker_directories.get("save_project").cloned();
                let default_name = this
                    .project_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "untitled.dawproj".to_string());

                cx.spawn(
                    async move |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .add_filter("DAW Project", &["dawproj"])
                            .set_title("Save Project As")
                            .set_file_name(default_name);
                        if let Some(dir) = start_dir {
                            dialog = dialog.set_directory(&dir);
                        }
                        let file = dialog.save_file().await;

                        if let Some(file) = file {
                            let path = file.path().to_path_buf();
                            let _ = cx.update(|cx| {
                                this.update(cx, |daw, _cx| {
                                    if let Some(parent) = path.parent() {
                                        daw.config.picker_directories.insert(
                                            "save_project".to_string(),
                                            parent.to_path_buf(),
                                        );
                                        daw.config.save();
                                    }
                                    if let Err(e) = daw.session.save(&path) {
                                        eprintln!("Failed to save project: {}", e);
                                    } else {
                                        daw.project_path = path;
                                    }
                                })
                            });
                        }
                    },
                )
                .detach();
            }))
            .on_action(cx.listener(|this, _: &RenderProject, _, cx| {
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
                                    if let Err(e) = daw.session.render_to_file(&path) {
                                        eprintln!("Failed to render: {}", e);
                                    }
                                })
                            });
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
                    // .child(cx.new(|_| Sidebar::new()))
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .bg(theme.elevated)
                            .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right(px(TRACK_LABEL_WIDTH))
                                    .bottom_0()
                                    .flex()
                                    .flex_col()
                                    .child(ruler)
                                    .child(
                                        div()
                                            .id("track_container")
                                            .flex_1()
                                            .overflow_scroll()
                                            .track_scroll(&self.scroll_handle)
                                            .child(
                                                div()
                                                    .min_w(px(timeline_width as f32))
                                                    .w_full()
                                                    .h_full()
                                                    .relative()
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                                                            let x_pos: f32 = event.position.x.into();
                                                            this.handle_timeline_click(x_pos as f64, cx);
                                                        }),
                                                    )
                                                    .child(self.render_grid_lines(pixels_per_beat, time_signature, timeline_width, cx))
                                                    .children(self.track_entities.iter().cloned())
                                                    .child(self.cursor_handle.clone())
                                                    .child(self.playhead_handle.clone()),
                                            ),
                                    ),
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
