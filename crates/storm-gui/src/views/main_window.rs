use crate::state::AppState;
use crate::theme::colors;
use gpui::*;

pub struct MainWindowView<'a> {
    state: &'a AppState,
}

impl<'a> MainWindowView<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
}

impl<'a> RenderOnce for MainWindowView<'a> {
    fn render(self, _cx: &mut Window) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::background())
            .text_color(colors::text_primary())
            .child(self.render_header())
            .child(self.render_content())
    }
}

impl<'a> MainWindowView<'a> {
    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(56.0))
            .px(px(16.0))
            .bg(colors::surface())
            .border_b_1()
            .border_color(rgba(0xffffff10))
            .child(
                div().flex().items_center().gap(px(8.0)).child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(colors::accent())
                        .child("StormDL"),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors::text_secondary())
                    .child(format!("{} downloads", self.state.downloads.len())),
            )
    }

    fn render_content(&self) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .child(self.render_sidebar())
            .child(self.render_main_panel())
    }

    fn render_sidebar(&self) -> impl IntoElement {
        div()
            .w(px(240.0))
            .h_full()
            .bg(colors::sidebar())
            .border_r_1()
            .border_color(rgba(0xffffff10))
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text_secondary())
                    .child("QUEUE"),
            )
            .children(self.state.downloads.iter().map(|d| {
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xffffff10)))
                    .child(div().text_sm().text_ellipsis().child(d.filename.clone()))
            }))
    }

    fn render_main_panel(&self) -> impl IntoElement {
        div().flex_1().p(px(16.0)).child(
            div()
                .text_color(colors::text_secondary())
                .child("Select a download to view details"),
        )
    }
}
