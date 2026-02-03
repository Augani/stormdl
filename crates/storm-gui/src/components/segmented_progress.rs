use adabraka_ui::prelude::*;
use gpui::*;
use stormdl_core::{SegmentState, SegmentStatus};

pub struct SegmentedProgressBar {
    segments: Vec<SegmentState>,
    height: Pixels,
}

impl SegmentedProgressBar {
    pub fn new(segments: Vec<SegmentState>) -> Self {
        Self {
            segments,
            height: px(24.0),
        }
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    fn segment_color(status: SegmentStatus) -> Hsla {
        match status {
            SegmentStatus::Pending => hsla(0.0, 0.0, 0.29, 1.0),
            SegmentStatus::Active => hsla(0.58, 1.0, 0.65, 1.0),
            SegmentStatus::Complete => hsla(0.39, 0.74, 0.58, 1.0),
            SegmentStatus::Error => hsla(0.0, 0.84, 0.6, 1.0),
            SegmentStatus::Slow => hsla(0.12, 0.98, 0.56, 1.0),
        }
    }
}

impl RenderOnce for SegmentedProgressBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = use_theme();

        if self.segments.is_empty() {
            return div()
                .w_full()
                .h(self.height)
                .rounded_sm()
                .bg(theme.tokens.muted)
                .into_any_element();
        }

        let total_len: u64 = self.segments.iter().map(|s| s.range.len()).sum();
        if total_len == 0 {
            return div()
                .w_full()
                .h(self.height)
                .rounded_sm()
                .bg(theme.tokens.primary)
                .into_any_element();
        }

        div()
            .w_full()
            .h(self.height)
            .rounded_sm()
            .overflow_hidden()
            .flex()
            .children(self.segments.iter().map(|segment| {
                let width_fraction = segment.range.len() as f32 / total_len as f32;
                let fill_fraction = segment.progress() as f32;

                div()
                    .flex_shrink_0()
                    .h_full()
                    .relative()
                    .w(relative(width_fraction))
                    .bg(theme.tokens.muted)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .h_full()
                            .w(relative(fill_fraction))
                            .bg(Self::segment_color(segment.status)),
                    )
            }))
            .into_any_element()
    }
}
