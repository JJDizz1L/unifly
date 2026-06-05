//! Redraw readiness decisions for the TUI event loop.

pub(super) fn should_draw(needs_redraw: bool, effects_active: bool) -> bool {
    should_draw_with_graphics_ready(needs_redraw, effects_active, graphics_ready())
}

pub(super) fn should_draw_with_graphics_ready(
    needs_redraw: bool,
    effects_active: bool,
    graphics_ready: bool,
) -> bool {
    needs_redraw || effects_active || graphics_ready
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
        assert!(!should_draw_with_graphics_ready(false, false, false));
    }

    #[test]
    fn redraw_request_draws() {
        assert!(should_draw_with_graphics_ready(true, false, false));
    }

    #[test]
    fn active_effect_draws() {
        assert!(should_draw_with_graphics_ready(false, true, false));
    }

    #[test]
    fn ready_graphics_draws() {
        assert!(should_draw_with_graphics_ready(false, false, true));
    }
}
