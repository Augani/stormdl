use crate::theme::colors;
use gpui::*;
use storm_core::{SegmentState, SegmentStatus};

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

    fn segment_color(status: SegmentStatus) -> Rgba {
        match status {
            SegmentStatus::Pending => colors::segment_pending(),
            SegmentStatus::Active => colors::segment_active(),
            SegmentStatus::Complete => colors::segment_complete(),
            SegmentStatus::Error => colors::segment_error(),
            SegmentStatus::Slow => colors::segment_slow(),
        }
    }
}

impl RenderOnce for SegmentedProgressBar {
    fn render(self, _cx: &mut Window) -> impl IntoElement {
        if self.segments.is_empty() {
            return div()
                .w_full()
                .h(self.height)
                .rounded_sm()
                .bg(colors::segment_pending())
                .into_any_element();
        }

        let total_len: u64 = self.segments.iter().map(|s| s.range.len()).sum();
        if total_len == 0 {
            return div()
                .w_full()
                .h(self.height)
                .rounded_sm()
                .bg(colors::segment_complete())
                .into_any_element();
        }

        div()
            .w_full()
            .h(self.height)
            .rounded_sm()
            .overflow_hidden()
            .flex()
            .children(self.segments.iter().map(|segment| {
                let width_pct = (segment.range.len() as f32 / total_len as f32) * 100.0;
                let fill_pct = segment.progress() as f32 * 100.0;

                div()
                    .flex_shrink_0()
                    .h_full()
                    .relative()
                    .style(|s| {
                        s.width = Some(Length::Definite(DefiniteLength::Fraction(
                            width_pct / 100.0,
                        )));
                    })
                    .bg(colors::segment_pending())
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .h_full()
                            .style(|s| {
                                s.width = Some(Length::Definite(DefiniteLength::Fraction(
                                    fill_pct / 100.0,
                                )));
                            })
                            .bg(Self::segment_color(segment.status)),
                    )
            }))
            .into_any_element()
    }
}
