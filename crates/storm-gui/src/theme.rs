use adabraka_ui::{install_theme, Theme};
use gpui::*;

pub fn install_storm_theme(cx: &mut App) {
    install_theme(cx, Theme::dark());
}

pub mod colors {
    use gpui::rgb;

    pub fn background() -> gpui::Rgba {
        rgb(0x1a1a2e).into()
    }

    pub fn surface() -> gpui::Rgba {
        rgb(0x16213e).into()
    }

    pub fn sidebar() -> gpui::Rgba {
        rgb(0x0f3460).into()
    }

    pub fn accent() -> gpui::Rgba {
        rgb(0xe94560).into()
    }

    pub fn text_primary() -> gpui::Rgba {
        rgb(0xeaeaea).into()
    }

    pub fn text_secondary() -> gpui::Rgba {
        rgb(0xa0a0a0).into()
    }

    pub fn segment_pending() -> gpui::Rgba {
        rgb(0x4a4a4a).into()
    }

    pub fn segment_active() -> gpui::Rgba {
        rgb(0x4a9fff).into()
    }

    pub fn segment_complete() -> gpui::Rgba {
        rgb(0x4ade80).into()
    }

    pub fn segment_error() -> gpui::Rgba {
        rgb(0xef4444).into()
    }

    pub fn segment_slow() -> gpui::Rgba {
        rgb(0xfbbf24).into()
    }
}
