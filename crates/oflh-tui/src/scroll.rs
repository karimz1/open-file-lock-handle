//! The shared scroll affordance: a scrollbar in the right column, a
//! "first–last of total ↕" indicator and a compact key hint. Everything is
//! drawn only while a region overflows, so views that fit stay unchanged.
use super::messages::{Language, localized};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    symbols::scrollbar,
    widgets::{Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use unicode_width::UnicodeWidthStr;

/// Which part of a list is visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub offset: usize,
    pub visible: usize,
    pub total: usize,
}

impl Window {
    pub fn overflows(self) -> bool {
        self.total > self.visible
    }
}

pub const HINT: &str = "↑↓ / PgUp PgDn scroll · Home/End";

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn indicator(language: Language, window: Window) -> String {
    localized!(
        language,
        "{first}–{last} of {total} ↕",
        first = window.offset + 1,
        last = (window.offset + window.visible).min(window.total),
        total = window.total
    )
}

/// A scrollbar over the right column of `rows`, the area the items occupy.
pub fn scrollbar(frame: &mut Frame, rows: Rect, window: Window) {
    if !window.overflows() || rows.width == 0 || rows.height == 0 {
        return;
    }
    let mut state = ScrollbarState::new(window.total - window.visible + 1)
        .position(window.offset)
        .viewport_content_length(window.visible);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .symbols(scrollbar::VERTICAL)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .track_style(dim()),
        rows,
        &mut state,
    );
}

/// Right-aligned text on one line, clearing only the cells it covers so a
/// title or column heading to its left stays readable.
fn right(frame: &mut Frame, line: Rect, text: &str) {
    let width = text.width() as u16;
    if width == 0 || width > line.width {
        return;
    }
    let area = Rect::new(line.right() - width, line.y, width, 1);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text.to_owned()).style(dim()), area);
}

/// The indicator on `line`. The key hint goes on `hint_line` when one is
/// free, otherwise it joins the indicator only if both stay compact; on
/// narrow screens the hint is dropped and the footer keys still apply.
pub fn indicator_line(
    frame: &mut Frame,
    line: Rect,
    hint_line: Option<Rect>,
    language: Language,
    window: Window,
) {
    if !window.overflows() {
        return;
    }
    let indicator = indicator(language, window);
    let hint = language.text(HINT);
    let half = line.width as usize / 2;
    if let Some(hint_line) = hint_line {
        right(frame, line, &indicator);
        if hint.width() + 2 <= hint_line.width as usize {
            right(frame, hint_line, hint)
        }
        return;
    }
    let joined = format!("{indicator} · {hint}");
    if joined.width() <= half {
        right(frame, line, &joined)
    } else {
        right(frame, line, &indicator)
    }
}

/// Scroll chrome for a table whose first row is its header.
pub fn table(frame: &mut Frame, area: Rect, language: Language, window: Window) {
    if area.height < 2 {
        return;
    }
    indicator_line(
        frame,
        Rect::new(area.x, area.y, area.width, 1),
        None,
        language,
        window,
    );
    scrollbar(
        frame,
        Rect::new(area.x, area.y + 1, area.width, area.height - 1),
        window,
    );
}

/// Scroll chrome for a bordered table: the indicator sits in the top border
/// and the scrollbar replaces the right border beside the rows.
pub fn bordered_table(frame: &mut Frame, area: Rect, language: Language, window: Window) {
    if area.height < 4 || area.width < 4 {
        return;
    }
    indicator_line(
        frame,
        Rect::new(area.x + 1, area.y, area.width - 2, 1),
        None,
        language,
        window,
    );
    scrollbar(
        frame,
        Rect::new(area.x, area.y + 2, area.width, area.height - 3),
        window,
    );
}
