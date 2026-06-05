//! Redraw readiness decisions for the TUI event loop.

pub(super) fn should_draw(needs_redraw: bool, effects_active: bool) -> bool {
    needs_redraw || effects_active || graphics_ready()
}

#[cfg(feature = "tui-graphics")]
fn graphics_ready() -> bool {
    crate::tui::graphics::has_ready_chart()
}

#[cfg(not(feature = "tui-graphics"))]
fn graphics_ready() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_clean_frame_does_not_draw() {
        assert!(!should_draw(false, false));
    }

    #[test]
    fn redraw_request_draws() {
        assert!(should_draw(true, false));
    }

    #[test]
    fn active_effect_draws() {
        assert!(should_draw(false, true));
    }
}
