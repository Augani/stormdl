use crate::state::{AppState, DownloadEvent, OrchestratorCommand};
use crate::theme::install_storm_theme;
use flume::{Receiver, Sender};
use gpui::*;
use std::path::PathBuf;

pub struct StormApp {
    state: AppState,
}

impl StormApp {
    pub fn new(
        command_tx: Sender<OrchestratorCommand>,
        event_rx: Receiver<DownloadEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = AppState::new(command_tx, event_rx.clone());

        cx.spawn(|this, mut cx| async move {
            while let Ok(event) = event_rx.recv_async().await {
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.handle_event(event, cx);
                    })
                });
            }
        })
        .detach();

        Self { state }
    }

    fn handle_event(&mut self, event: DownloadEvent, cx: &mut Context<Self>) {
        match event {
            DownloadEvent::DownloadAdded { id, url, filename, total_size } => {
                self.state.add_download(id, url, filename, total_size);
            }
            DownloadEvent::ProgressUpdate { id, downloaded, segments } => {
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
            DownloadEvent::SegmentRebalanced { id, new_count, .. } => {
                tracing::info!("Download {} rebalanced to {} segments", id.0, new_count);
            }
            DownloadEvent::Error { id, error } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.error = Some(error);
                    download.state = storm_core::DownloadState::Failed;
                }
            }
            DownloadEvent::Complete { id, path, hash } => {
                if let Some(download) = self.state.get_download_mut(id) {
                    download.state = storm_core::DownloadState::Complete;
                    tracing::info!("Download {} complete: {:?} ({})", id.0, path, hash);
                }
            }
        }
        cx.notify();
    }
}

impl Render for StormApp {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1a1a2e))
            .text_color(rgb(0xeaeaea))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(60.0))
                    .bg(rgb(0x16213e))
                    .child("StormDL"),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(
                        div()
                            .w(px(250.0))
                            .bg(rgb(0x0f3460))
                            .child("Downloads"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p(px(16.0))
                            .child(format!("{} downloads", self.state.downloads.len())),
                    ),
            )
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
            install_storm_theme(cx);

            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

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
