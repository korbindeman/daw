mod app_menus;
mod keybindings;
mod theme;
mod ui;

use app_menus::{OpenProject, RenderProject, app_menus};
use daw_core::Session;
use gpui::{
    App, Application, Context, Entity, FocusHandle, Timer, Window, WindowOptions, actions, div,
    prelude::*, px,
};
use keybindings::keybindings;
use std::path::{Path, PathBuf};
use std::time::Duration;
use theme::ActiveTheme;
use ui::{Header, HeaderEvent, Playhead, Sidebar, TimelineRuler, Track, TrackLabels};

struct Daw {
    session: Session,
    header_handle: Entity<Header>,
    playhead_handle: Entity<Playhead>,
    focus_handle: FocusHandle,
    project_path: PathBuf,
}

impl Daw {
    fn new(cx: &mut Context<Self>) -> Self {
        Self::from_path(Path::new("projects/test.dawproj"), cx)
    }

    fn from_path(path: &Path, cx: &mut Context<Self>) -> Self {
        let session = Session::from_project(path).expect("Failed to load project");

        let time_signature = session.time_signature();
        let tempo = session.tempo();

        let header = cx.new(|cx| Header::new(0, time_signature.into(), tempo, cx));
        cx.subscribe(
            &header,
            |this, header, event: &HeaderEvent, cx| match event {
                HeaderEvent::Play => this.play(&header, cx),
                HeaderEvent::Pause => this.pause(&header, cx),
                HeaderEvent::Stop => this.stop(&header, cx),
            },
        )
        .detach();

        let pixels_per_beat = session.time_context().pixels_per_beat;
        let playhead = cx.new(|_| Playhead::new(0, pixels_per_beat));

        cx.spawn(
            async |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                loop {
                    Timer::after(Duration::from_millis(16)).await;
                    let result = cx.update(|cx| {
                        this.update(cx, |daw, cx| {
                            daw.poll_status(cx);
                        })
                    });
                    if result.is_err() {
                        break;
                    }
                }
            },
        )
        .detach();

        let focus_handle = cx.focus_handle();

        Self {
            session,
            header_handle: header,
            playhead_handle: playhead,
            focus_handle,
            project_path: path.to_path_buf(),
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
                // Update session
                self.session = session;
                self.project_path = path;

                // Update header
                self.header_handle.update(cx, |header, cx| {
                    header.set_tick(0, cx);
                    header.set_playing(false, cx);
                });

                // Update playhead
                self.playhead_handle.update(cx, |playhead, cx| {
                    playhead.set_tick(0);
                    cx.notify();
                });

                cx.notify();
            }
            Err(e) => {
                eprintln!("Failed to load project: {}", e);
            }
        }
    }

    fn poll_status(&mut self, cx: &mut Context<Self>) {
        if let Some(tick) = self.session.poll() {
            self.header_handle.update(cx, |header, cx| {
                header.set_tick(tick, cx);
            });
            self.playhead_handle.update(cx, |playhead, cx| {
                playhead.set_tick(tick);
                cx.notify();
            });
        }
    }

    fn play(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.play();
        header.update(cx, |header, cx| header.set_playing(true, cx));
    }

    fn pause(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.pause();
        header.update(cx, |header, cx| header.set_playing(false, cx));
    }

    fn stop(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        self.session.stop();
        header.update(cx, |header, cx| header.set_playing(false, cx));
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
            .on_action(cx.listener(|_this, _: &OpenProject, _, cx| {
                cx.spawn(
                    async |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("DAW Project", &["dawproj"])
                            .set_title("Open Project")
                            .pick_file()
                            .await;

                        if let Some(file) = file {
                            let path = file.path().to_path_buf();
                            let _ = cx.update(|cx| {
                                this.update(cx, |daw, cx| {
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

                cx.spawn(
                    async move |_this: gpui::WeakEntity<Self>, _cx: &mut gpui::AsyncApp| {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("WAV Audio", &["wav"])
                            .set_title("Render to WAV")
                            .set_file_name("render.wav")
                            .save_file()
                            .await;

                        if let Some(file) = file {
                            let path = file.path().to_path_buf();
                            let buffer = daw_render::render_timeline(&tracks, tempo, 44100, 2);
                            if let Err(e) = daw_render::write_wav(&buffer, &path) {
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
                            .overflow_hidden()
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
                                    .child(
                                        div()
                                            .flex_1()
                                            .relative()
                                            .child(self.playhead_handle.clone())
                                            .children(tracks.iter().map(|track| {
                                                cx.new(|_| {
                                                    Track::new(
                                                        track.clone(),
                                                        pixels_per_beat,
                                                        self.session.tempo(),
                                                        timeline_width,
                                                    )
                                                })
                                            })),
                                    ),
                            )
                            .child(cx.new(|_| TrackLabels::new(tracks.to_vec()))),
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

        // Open window
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|cx| Daw::new(cx)))
            .unwrap();
    });
}
