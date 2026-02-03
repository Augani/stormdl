use crate::state::{AppState, DownloadEvent, OrchestratorCommand};
use adabraka_ui::components::button::{Button, ButtonVariant};
use adabraka_ui::components::icon::Icon;
use adabraka_ui::components::input::{Input, InputState};
use adabraka_ui::components::progress::ProgressBar;
use adabraka_ui::components::scrollable::scrollable_vertical;
use adabraka_ui::components::spinner::Spinner;
use adabraka_ui::display::badge::{Badge, BadgeVariant};
use adabraka_ui::prelude::*;
use flume::{Receiver, Sender};
use gpui::*;
use std::path::PathBuf;
use stormdl_core::{DownloadOptions, DownloadState};
use url::Url;

pub struct StormApp {
    state: AppState,
    url_input: Entity<InputState>,
    save_location: PathBuf,
}

impl StormApp {
    pub fn new(
        command_tx: Sender<OrchestratorCommand>,
        event_rx: Receiver<DownloadEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = AppState::new(command_tx, event_rx.clone());
        let url_input = cx.new(InputState::new);
        let save_location = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));

        cx.spawn(async move |this, cx| {
            while let Ok(event) = event_rx.recv_async().await {
                let _ = this.update(cx, |app, cx| {
                    app.handle_event(event, cx);
                });
            }
        })
        .detach();

        Self {
            state,
            url_input,
            save_location,
        }
    }

    fn handle_event(&mut self, event: DownloadEvent, cx: &mut Context<Self>) {
        match event {
            DownloadEvent::DownloadAdded {
                id,
                url,
                filename,
                total_size,
            } => {
                self.state.add_download(id, url, filename, total_size);
            }
            DownloadEvent::ProgressUpdate {
                id,
                downloaded,
                segments,
            } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.downloaded_bytes = downloaded;
                    download.segments = segments;
                }
            }
            DownloadEvent::SpeedUpdate { id, speed } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.add_speed_sample(speed);
                }
            }
            DownloadEvent::StateChange { id, state } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.state = state;
                }
            }
            DownloadEvent::SegmentRebalanced { .. } => {}
            DownloadEvent::Error { id, error } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.error = Some(error);
                    download.state = DownloadState::Failed;
                }
            }
            DownloadEvent::Complete { id, .. } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.state = DownloadState::Complete;
                }
            }
        }
        cx.notify();
    }

    fn start_download(&mut self, cx: &mut Context<Self>) {
        let url_str = self.url_input.read(cx).content.to_string();
        if url_str.trim().is_empty() {
            return;
        }
        if let Ok(url) = Url::parse(&url_str) {
            let options = DownloadOptions {
                url: url.clone(),
                output_dir: self.save_location.clone(),
                filename: None,
                segments: None,
                priority: stormdl_core::Priority::Normal,
                bandwidth_limit: None,
                headers: vec![],
                checksum: None,
            };

            let _ = self
                .state
                .command_tx
                .send(OrchestratorCommand::AddDownload { url, options });
            self.url_input.update(cx, |input, _| {
                input.content = SharedString::default();
            });
            cx.notify();
        }
    }

    fn browse_location(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx.update(|cx| {
                cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: None,
                })
            });

            if let Ok(receiver) = result {
                if let Ok(Ok(Some(paths))) = receiver.await {
                    if let Some(selected) = paths.into_iter().next() {
                        let _ = this.update(cx, |app, cx| {
                            app.save_location = selected;
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();
    }
}

impl Render for StormApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = use_theme();

        div()
            .size_full()
            .bg(theme.tokens.background)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(24.0))
                    .py(px(20.0))
                    .bg(theme.tokens.primary.opacity(0.05))
                    .border_b_1()
                    .border_color(theme.tokens.border)
                    .child(
                        div()
                            .size(px(40.0))
                            .rounded_full()
                            .bg(theme.tokens.primary.opacity(0.15))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new("zap").size(px(20.0)).color(theme.tokens.primary)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(20.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.tokens.foreground)
                                            .child("StormDL"),
                                    )
                                    .child(Badge::new("v0.1.0").variant(BadgeVariant::Secondary)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.tokens.muted_foreground)
                                    .child("Lightning-fast parallel downloads"),
                            ),
                    ),
            )
            .child(
                div().flex_1().overflow_hidden().child(scrollable_vertical(
                    div()
                        .p(px(24.0))
                        .flex()
                        .flex_col()
                        .gap(px(20.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.tokens.foreground)
                                        .child("Download URL"),
                                )
                                .child(
                                    Input::new(&self.url_input)
                                        .placeholder("https://example.com/file.zip")
                                        .prefix(
                                            Icon::new("link")
                                                .size(px(16.0))
                                                .color(theme.tokens.muted_foreground),
                                        )
                                        .clearable(true),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.tokens.foreground)
                                        .child("Save to"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .flex_1()
                                                .h(px(40.0))
                                                .px(px(12.0))
                                                .bg(theme.tokens.muted.opacity(0.3))
                                                .border_1()
                                                .border_color(theme.tokens.border)
                                                .rounded(theme.tokens.radius_md)
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(
                                                    Icon::new("folder")
                                                        .size(px(16.0))
                                                        .color(theme.tokens.muted_foreground),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(14.0))
                                                        .text_color(theme.tokens.foreground)
                                                        .text_ellipsis()
                                                        .overflow_hidden()
                                                        .child(
                                                            self.save_location
                                                                .to_string_lossy()
                                                                .to_string(),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            Button::new("browse", "Browse")
                                                .variant(ButtonVariant::Ghost)
                                                .icon("folder-open")
                                                .on_click(cx.listener(|this, _, _window, cx| {
                                                    this.browse_location(cx);
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            Button::new("download", "Download")
                                .icon("download")
                                .variant(ButtonVariant::Default)
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.start_download(cx);
                                })),
                        )
                        .child(self.render_downloads_list()),
                )),
            )
    }
}

impl StormApp {
    fn render_downloads_list(&self) -> impl IntoElement {
        let theme = use_theme();

        if self.state.downloads.is_empty() {
            return div().into_any_element();
        }

        let download_items: Vec<_> = self
            .state
            .downloads
            .iter()
            .rev()
            .map(|download| {
                let progress = download.progress();
                let speed = download.current_speed();
                let state = download.state;
                let filename = download.filename.clone();
                let downloaded = download.downloaded_bytes;
                let total = download.total_bytes;
                let error = download.error.clone();

                let state_text = match state {
                    DownloadState::Pending => "Pending",
                    DownloadState::Probing => "Probing",
                    DownloadState::Downloading => "Downloading",
                    DownloadState::Paused => "Paused",
                    DownloadState::Complete => "Complete",
                    DownloadState::Failed => "Failed",
                    DownloadState::Cancelled => "Cancelled",
                };

                let badge_variant = if state == DownloadState::Complete {
                    BadgeVariant::Secondary
                } else if state == DownloadState::Failed {
                    BadgeVariant::Destructive
                } else {
                    BadgeVariant::Outline
                };

                let status_icon =
                    if state == DownloadState::Downloading || state == DownloadState::Probing {
                        Spinner::new().into_any_element()
                    } else if state == DownloadState::Complete {
                        Icon::new("circle-check")
                            .size(px(18.0))
                            .color(theme.tokens.primary)
                            .into_any_element()
                    } else if state == DownloadState::Failed {
                        Icon::new("circle-x")
                            .size(px(18.0))
                            .color(theme.tokens.destructive)
                            .into_any_element()
                    } else {
                        Icon::new("file")
                            .size(px(18.0))
                            .color(theme.tokens.muted_foreground)
                            .into_any_element()
                    };

                let speed_display = if state == DownloadState::Downloading && speed > 0.0 {
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.tokens.primary)
                        .child(format!("{}/s", bytesize::ByteSize(speed as u64)))
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                let error_display = if let Some(err) = error {
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.tokens.destructive)
                        .child(err)
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                div()
                    .p(px(16.0))
                    .bg(theme.tokens.card)
                    .border_1()
                    .border_color(theme.tokens.border)
                    .rounded(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(status_icon)
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.tokens.foreground)
                                            .text_ellipsis()
                                            .overflow_hidden()
                                            .max_w(px(280.0))
                                            .child(filename),
                                    ),
                            )
                            .child(Badge::new(state_text).variant(badge_variant)),
                    )
                    .child(ProgressBar::new(progress as f32))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.tokens.muted_foreground)
                                    .child(format!(
                                        "{} / {}",
                                        bytesize::ByteSize(downloaded),
                                        total
                                            .map(|t| bytesize::ByteSize(t).to_string())
                                            .unwrap_or("?".into())
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(speed_display)
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.tokens.foreground)
                                            .child(format!("{:.1}%", progress * 100.0)),
                                    ),
                            ),
                    )
                    .child(error_display)
            })
            .collect();

        div()
            .mt(px(16.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .children(download_items)
            .into_any_element()
    }
}

struct Assets {
    base_path: PathBuf,
}

impl Assets {
    fn new() -> Self {
        let base_path = if cfg!(debug_assertions) {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
        } else {
            #[cfg(target_os = "macos")]
            {
                PathBuf::from("../Resources")
            }
            #[cfg(not(target_os = "macos"))]
            {
                PathBuf::from("assets")
            }
        };
        Self { base_path }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let full_path = self.base_path.join(path);
        match std::fs::read(&full_path) {
            Ok(data) => Ok(Some(std::borrow::Cow::Owned(data))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let full_path = self.base_path.join(path);
        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&full_path) {
            for entry in dir.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    entries.push(SharedString::from(name.to_string()));
                }
            }
        }
        Ok(entries)
    }
}

pub fn run_app(command_tx: Sender<OrchestratorCommand>, event_rx: Receiver<DownloadEvent>) {
    Application::new()
        .with_assets(Assets::new())
        .run(move |cx| {
            adabraka_ui::init(cx);
            adabraka_ui::set_icon_base_path("icons");
            install_theme(cx, Theme::dark());

            let bounds = Bounds::centered(None, size(px(500.0), px(450.0)), cx);

            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("StormDL")),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    let command_tx = command_tx.clone();
                    let event_rx = event_rx.clone();
                    cx.new(|cx| StormApp::new(command_tx, event_rx, cx))
                },
            )
            .unwrap();
        });
}
