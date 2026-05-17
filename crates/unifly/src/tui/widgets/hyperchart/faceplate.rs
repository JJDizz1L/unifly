//! Switch faceplate widget for device port detail views.

use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unifly_api::model::{Port, PortConnector, PortState};

use crate::tui::theme;

pub struct SwitchFaceplate<'a> {
    ports: &'a [Port],
    empty_message: &'a str,
}

impl<'a> SwitchFaceplate<'a> {
    pub const fn new(ports: &'a [Port]) -> Self {
        Self {
            ports,
            empty_message: "No port data available",
        }
    }

    #[must_use]
    pub fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }
}

impl Widget for SwitchFaceplate<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.ports.is_empty() {
            Paragraph::new(self.empty_message)
                .style(Style::default().fg(theme::text_muted()))
                .render(area, buf);
            return;
        }

        let by_index: BTreeMap<u32, &Port> =
            self.ports.iter().map(|port| (port.index, port)).collect();
        let max_index = by_index.keys().copied().max().unwrap_or(0);
        let pair_count = max_index.div_ceil(2).max(1);
        let pairs_per_bank = usize::from(area.width.saturating_sub(2) / 4).max(1);
        let mut lines = Vec::new();

        for bank_start in (1..=pair_count).step_by(pairs_per_bank) {
            let bank_end =
                (bank_start + u32::try_from(pairs_per_bank).unwrap_or(1) - 1).min(pair_count);
            lines.push(label_line(&by_index, bank_start, bank_end, true));
            lines.push(glyph_line(&by_index, bank_start, bank_end, true));
            lines.push(glyph_line(&by_index, bank_start, bank_end, false));
            lines.push(label_line(&by_index, bank_start, bank_end, false));
        }

        lines.push(Line::from(vec![
            Span::styled("  █ 1G+  ", Style::default().fg(theme::success())),
            Span::styled("▓ 100M  ", Style::default().fg(theme::warning())),
            Span::styled("░ down  ", Style::default().fg(theme::text_muted())),
            Span::styled("⇅ uplink  ", Style::default().fg(theme::accent_primary())),
            Span::styled("⚡ PoE", Style::default().fg(theme::accent_secondary())),
        ]));

        Paragraph::new(lines).render(area, buf);
    }
}

fn label_line(
    ports: &BTreeMap<u32, &Port>,
    pair_start: u32,
    pair_end: u32,
    odd: bool,
) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for pair in pair_start..=pair_end {
        let index = if odd { pair * 2 - 1 } else { pair * 2 };
        let label = ports
            .get(&index)
            .map_or_else(|| " ".to_string(), |port| port.index.to_string());
        spans.push(Span::styled(
            format!("{label:^4}"),
            Style::default().fg(theme::text_secondary()),
        ));
    }
    Line::from(spans)
}

fn glyph_line(
    ports: &BTreeMap<u32, &Port>,
    pair_start: u32,
    pair_end: u32,
    odd: bool,
) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for pair in pair_start..=pair_end {
        let index = if odd { pair * 2 - 1 } else { pair * 2 };
        let Some(port) = ports.get(&index) else {
            spans.push(Span::raw("    "));
            continue;
        };
        let body = if is_uplink(port) {
            "⇅"
        } else {
            port_glyph(port)
        };
        let shell = if odd {
            format!("▟{body}▙")
        } else {
            format!("▜{body}▛")
        };
        spans.push(Span::styled(
            format!("{shell:^4}"),
            port_style(port).add_modifier(poe_modifier(port)),
        ));
    }
    Line::from(spans)
}

fn is_uplink(port: &Port) -> bool {
    matches!(
        port.connector,
        Some(
            PortConnector::Sfp
                | PortConnector::SfpPlus
                | PortConnector::Sfp28
                | PortConnector::Qsfp28
        )
    )
}

fn port_glyph(port: &Port) -> &'static str {
    match port.state {
        PortState::Up if port.speed_mbps.unwrap_or_default() >= 1_000 => "█",
        PortState::Up if port.speed_mbps.unwrap_or_default() >= 100 => "▓",
        PortState::Up => "▒",
        PortState::Down => "░",
        PortState::Unknown => "·",
    }
}

fn port_style(port: &Port) -> Style {
    let color = match port.state {
        PortState::Down => theme::text_muted(),
        PortState::Unknown => theme::border_unfocused(),
        PortState::Up if is_uplink(port) => theme::accent_primary(),
        PortState::Up if port.speed_mbps.unwrap_or_default() >= 1_000 => theme::success(),
        PortState::Up if port.speed_mbps.unwrap_or_default() >= 100 => theme::warning(),
        PortState::Up => theme::accent_secondary(),
    };
    Style::default().fg(color)
}

fn poe_modifier(port: &Port) -> Modifier {
    if port.poe.as_ref().is_some_and(|poe| poe.enabled) {
        Modifier::BOLD
    } else {
        Modifier::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unifly_api::model::PoeInfo;

    fn port(index: u32, state: PortState, speed_mbps: Option<u32>) -> Port {
        Port {
            index,
            name: None,
            state,
            speed_mbps,
            max_speed_mbps: Some(1_000),
            connector: Some(PortConnector::Rj45),
            poe: None,
        }
    }

    #[test]
    fn port_glyph_tracks_link_state_and_speed() {
        assert_eq!(port_glyph(&port(1, PortState::Up, Some(1_000))), "█");
        assert_eq!(port_glyph(&port(2, PortState::Up, Some(100))), "▓");
        assert_eq!(port_glyph(&port(3, PortState::Down, None)), "░");
    }

    #[test]
    fn poe_ports_render_bold() {
        let mut port = port(1, PortState::Up, Some(1_000));
        port.poe = Some(PoeInfo {
            standard: Some("PoE+".into()),
            enabled: true,
            state: PortState::Up,
        });

        assert!(poe_modifier(&port).contains(Modifier::BOLD));
    }
}
