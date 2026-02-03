use crate::state::Download;
use crate::theme::colors;
use gpui::*;

pub struct DownloadListView {
    downloads: Vec<Download>,
}

impl DownloadListView {
    pub fn new(downloads: Vec<Download>) -> Self {
        Self { downloads }
    }
}

impl RenderOnce for DownloadListView {
    fn render(self, _cx: &mut Window) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .children(self.downloads.into_iter().map(|download| {
                div()
                    .p(px(12.0))
                    .rounded_md()
                    .bg(colors::surface())
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors::text_primary())
                            .child(download.filename.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors::text_secondary())
                            .child(format!(
                                "{:.1}% - {:.2} MB/s",
                                download.progress() * 100.0,
                                download.current_speed() / 1_000_000.0
                            )),
                    )
            }))
    }
}
