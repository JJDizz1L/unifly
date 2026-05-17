//! tachyonfx scene transition effects for unifly.
//!
//! Wraps [`tachyonfx::EffectManager`] to provide a unifly-flavoured effect
//! stack keyed by [`EffectKind`]. Ported from chromacat's `src/renderer/effects.rs`
//! which is the in-house reference implementation of this pattern.
//!
//! The stack is applied as buffer post-processing inside the App's draw
//! closure: after screens, tabs, and the status bar render normally, any
//! active effects are applied to the frame buffer before overlay chrome
//! (notifications, dialogs, help/about) lands on top. This ordering means
//! effects animate the primary content without obscuring transient UI.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::{
    Duration as FxDuration, Effect, EffectManager, EffectTimer, Interpolation, Motion, fx,
    pattern::RadialPattern,
};

/// The role of a running effect — used to cancel in-progress effects of the
/// same kind when a new one of the same role is registered.
///
/// `EffectManager<K>` requires `K: Clone + Ord + Debug + Default`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectKind {
    /// One-shot reveal played on TUI launch.
    #[default]
    Intro,
    /// Screen-switch transition (dissolve → coalesce).
    ScreenTransition,
    /// Notification toast fade-out.
    NotificationFade,
    /// Theme swap fade.
    ThemeSwap,
    /// Chart peak / threshold pulse.
    ChartPulse,
    /// Topology node state-change ripple.
    TopologyPulse,
}

/// Manages active tachyonfx effects on the TUI frame buffer.
pub struct EffectStack {
    manager: EffectManager<EffectKind>,
}

impl Default for EffectStack {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectStack {
    pub fn new() -> Self {
        Self {
            manager: EffectManager::default(),
        }
    }

    /// Play the one-shot launch reveal — a top-to-bottom sweep from black.
    /// Subsequent calls cancel any prior intro that is still running.
    pub fn start_intro(&mut self) {
        let effect = fx::sweep_in(
            Motion::UpToDown,
            8,
            2,
            Color::Black,
            FxDuration::from_millis(1_200),
        );
        self.manager.add_unique_effect(EffectKind::Intro, effect);
    }

    /// Play a subtle foreground fade when the active screen changes.
    /// A 160ms `fade_from_fg` from black is snappy enough to feel
    /// responsive while staying calmer than a coalesce scatter — the
    /// new screen's text/borders fade up from black on the first few
    /// frames and the transition is done before you finish blinking.
    pub fn start_screen_transition(&mut self) {
        let effect = fx::fade_from_fg(Color::Black, FxDuration::from_millis(160));
        self.manager
            .add_unique_effect(EffectKind::ScreenTransition, effect);
    }

    /// Play a restrained pulse when chart data establishes a new peak.
    pub fn start_chart_pulse(&mut self) {
        let effect = fx::ping_pong(
            fx::lighten_fg(0.18, EffectTimer::from_ms(180, Interpolation::SineOut))
                .with_pattern(RadialPattern::center().with_transition_width(10.0)),
        );
        self.manager
            .add_unique_effect(EffectKind::ChartPulse, effect);
    }

    /// Register a custom effect under the given kind, cancelling any prior
    /// effect of the same kind.
    pub fn add_unique(&mut self, kind: EffectKind, effect: Effect) {
        self.manager.add_unique_effect(kind, effect);
    }

    /// Apply all running effects to the given buffer for a single frame.
    /// `delta` is the time elapsed since the previous frame.
    pub fn process(&mut self, delta: Duration, buf: &mut Buffer, area: Rect) {
        let fx_delta = FxDuration::from_secs_f32(delta.as_secs_f32());
        self.manager.process_effects(fx_delta, buf, area);
    }

    /// Whether any effect is currently running.
    pub fn is_active(&self) -> bool {
        self.manager.is_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_stack_starts_inactive() {
        let stack = EffectStack::new();
        assert!(!stack.is_active());
    }

    #[test]
    fn intro_activates_stack() {
        let mut stack = EffectStack::new();
        stack.start_intro();
        assert!(stack.is_active());
    }

    #[test]
    fn screen_transition_activates_stack() {
        let mut stack = EffectStack::new();
        stack.start_screen_transition();
        assert!(stack.is_active());
    }

    #[test]
    fn chart_pulse_activates_stack() {
        let mut stack = EffectStack::new();
        stack.start_chart_pulse();
        assert!(stack.is_active());
    }

    #[test]
    fn effect_kind_default_is_intro() {
        assert_eq!(EffectKind::default(), EffectKind::Intro);
    }

    #[test]
    fn effect_kind_ordering_is_stable() {
        assert!(EffectKind::Intro < EffectKind::ScreenTransition);
        assert!(EffectKind::ScreenTransition < EffectKind::NotificationFade);
        assert!(EffectKind::ChartPulse < EffectKind::TopologyPulse);
    }

    #[test]
    fn process_with_no_effects_is_safe() {
        let mut stack = EffectStack::new();
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        stack.process(Duration::from_millis(33), &mut buf, area);
        // No panic, no state change.
        assert!(!stack.is_active());
    }
}
