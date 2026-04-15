pub mod billable_view;
pub mod day_view;
pub mod project_tracking_view;
pub mod settings_view;
pub mod stats_view;
pub mod vacation_view;

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use crate::app::{Message, ACCENT, DANGER, FONT_MEDIUM, FONT_REGULAR, FONT_SEMIBOLD, SURFACE, SURFACE_HOVER, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY};

// ── Layout constants ────────────────────────────────────────────────────────

/// Standard page padding: 12px on top/left/right, 0 on bottom (scrollable handles bottom).
pub const PAGE_PADDING: Padding = Padding { top: 12.0, right: 12.0, bottom: 0.0, left: 12.0 };
/// Standard section gap between major content blocks.
pub const SECTION_GAP: f32 = 12.0;
/// Standard gap between list rows.
pub const LIST_ROW_SPACING: f32 = 4.0;
/// Spacing between a field label and its input in paired `column![label, input]` patterns.
pub const FORM_FIELD_GAP: f32 = 4.0;

// ── Date helpers ─────────────────────────────────────────────────────────────

/// Format decimal hours as `h:mm` (e.g. 6.6 → "6:36", -0.25 → "-0:15").
pub fn fmt_hm(hours: f64) -> String {
    let sign = if hours < 0.0 { "-" } else { "" };
    let total_mins = (hours.abs() * 60.0).round() as u32;
    let h = total_mins / 60;
    let m = total_mins % 60;
    format!("{sign}{h}:{m:02}")
}

pub fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    }
}

pub fn month_abbr(m: u32) -> &'static str {
    match m {
        1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr",
        5 => "May", 6 => "Jun", 7 => "Jul", 8 => "Aug",
        9 => "Sep", 10 => "Oct", 11 => "Nov", _ => "Dec",
    }
}

// ── Time helpers ──────────────────────────────────────────────────────────────

/// Convert decimal hours to "H:MM" string (e.g. 2.5 → "2:30").
pub fn format_hhmm(hours: f64) -> String {
    let total_minutes = (hours * 60.0).round() as u64;
    let h = total_minutes / 60;
    let m = total_minutes % 60;
    format!("{h}:{m:02}")
}

/// Parse a time string that is either "H:MM" or a decimal number.
/// Returns `None` if the value is ≤ 0 or unparseable.
pub fn parse_hours(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    if let Some(pos) = s.find(':') {
        let h: f64 = s[..pos].parse().ok()?;
        let m: f64 = s[pos + 1..].parse().ok()?;
        if !(0.0..60.0).contains(&m) {
            return None;
        }
        let v = h + m / 60.0;
        return if v > 0.0 { Some(v) } else { None };
    }
    let v: f64 = s.parse().ok()?;
    if v > 0.0 { Some(v) } else { None }
}

// ── Shared style functions ────────────────────────────────────────────────────

/// Return `c` with its alpha channel replaced by `a`.
pub fn with_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

pub fn field_label(label: &str) -> Element<'_, Message> {
    text(label).font(FONT_MEDIUM).size(12).color(TEXT_MUTED).into()
}

pub fn caption(content: impl std::fmt::Display) -> iced::widget::Text<'static> {
    text(content.to_string()).font(FONT_REGULAR).size(12).color(TEXT_MUTED)
}

/// Standard `SURFACE_RAISED` container background with the given corner radius.
pub fn raised_container_style(radius: f32) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(SURFACE_RAISED)),
        border: Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn input_style(
    theme: &iced::Theme,
    status: text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();
    let base = text_input::default(theme, status);
    text_input::Style {
        background: iced::Background::Color(crate::app::BACKGROUND),
        border: Border {
            color: if matches!(status, text_input::Status::Focused { .. }) {
                ACCENT
            } else {
                palette.background.strong.color
            },
            width: 1.5,
            radius: 8.0.into(),
        },
        value: base.value,
        placeholder: base.placeholder,
        selection: base.selection,
        icon: base.icon,
    }
}

pub fn accent_btn_style(
    _: &iced::Theme,
    status: button::Status,
) -> button::Style {
    let alpha = match status {
        button::Status::Hovered => 0.85,
        button::Status::Pressed => 0.7,
        _ => 1.0,
    };
    button::Style {
        background: Some(iced::Background::Color(with_alpha(ACCENT, alpha))),
        text_color: Color::WHITE,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Shared widgets ────────────────────────────────────────────────────────────

pub fn ghost_btn_style(
    _: &iced::Theme,
    status: button::Status,
) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Hovered => Color {
                r: SURFACE_RAISED.r + 0.04,
                g: SURFACE_RAISED.g + 0.04,
                b: SURFACE_RAISED.b + 0.04,
                a: 1.0,
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: TEXT_MUTED,
        border: Border { radius: 6.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// Completely transparent button — no border, no bg, no padding side-effects.
pub fn plain_btn_style(_: &iced::Theme, _: button::Status) -> button::Style {
    button::Style {
        background: None,
        ..Default::default()
    }
}

/// A proportional progress bar built from two `FillPortion` containers.
///
/// `pct` — fill fraction in `[0.0, 1.0]`.
/// `bar_color` — colour of the filled segment.
/// `height` — track height in logical pixels.
pub fn progress_bar(
    pct: f32,
    bar_color: Color,
    height: u16,
) -> Element<'static, Message> {
    let fill_portions = (pct * 1000.0) as u16;
    let empty_portions = 1000u16.saturating_sub(fill_portions);
    let radius = (height as f32 / 2.0).into();

    let fill: Element<Message> = if fill_portions > 0 {
        container(Space::new())
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(bar_color)),
                border: Border { radius, ..Default::default() },
                ..Default::default()
            })
            .width(Length::FillPortion(fill_portions))
            .height(height as f32)
            .into()
    } else {
        Space::new().width(Length::FillPortion(1)).height(Length::FillPortion(1)).into()
    };

    let empty: Element<Message> = if empty_portions > 0 {
        container(Space::new())
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(SURFACE_RAISED)),
                border: Border { radius, ..Default::default() },
                ..Default::default()
            })
            .width(Length::FillPortion(empty_portions))
            .height(height as f32)
            .into()
    } else {
        Space::new().width(Length::FillPortion(1)).height(Length::FillPortion(1)).into()
    };

    container(row![fill, empty].spacing(0))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(SURFACE_RAISED)),
            border: Border { radius, ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(height as f32)
        .into()
}

/// Small "×" delete button with DANGER hover.
pub fn delete_chip_btn(msg: Message) -> Element<'static, Message> {
    button(text("×").font(FONT_MEDIUM).size(13).color(TEXT_MUTED))
        .style(|_, status| button::Style {
            background: Some(iced::Background::Color(match status {
                button::Status::Hovered => with_alpha(DANGER, 0.15),
                _ => Color::TRANSPARENT,
            })),
            text_color: match status {
                button::Status::Hovered => DANGER,
                _ => TEXT_MUTED,
            },
            border: Border { radius: 4.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([2, 7])
        .on_press(msg)
        .into()
}

pub fn suggestion_btn_style(
    _: &iced::Theme,
    status: button::Status,
) -> button::Style {
    let bg = match status {
        button::Status::Hovered => SURFACE_HOVER,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: TEXT_PRIMARY,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Shared button styles ─────────────────────────────────────────────────────

pub fn outline_btn_style(_: &iced::Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    button::Style {
        background: Some(iced::Background::Color(if hovered {
            Color { r: SURFACE_RAISED.r + 0.03, g: SURFACE_RAISED.g + 0.03, b: SURFACE_RAISED.b + 0.03, a: 1.0 }
        } else {
            Color::TRANSPARENT
        })),
        text_color: TEXT_MUTED,
        border: Border {
            color: SURFACE_RAISED,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

pub fn danger_btn_style(
    _: &iced::Theme,
    status: button::Status,
) -> button::Style {
    let alpha = match status {
        button::Status::Hovered => 0.85,
        button::Status::Pressed => 0.7,
        _ => 1.0,
    };
    button::Style {
        background: Some(iced::Background::Color(with_alpha(DANGER, alpha))),
        text_color: Color::WHITE,
        border: Border { radius: 6.0.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn toggle_active_style(radius: f32) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(ACCENT)),
        text_color: Color::WHITE,
        border: Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn toggle_inactive_style(radius: f32) -> button::Style {
    button::Style {
        background: Some(iced::Background::Color(SURFACE_RAISED)),
        text_color: TEXT_MUTED,
        border: Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    }
}

// ── Button factories ─────────────────────────────────────────────────────────

/// Regular primary — FONT_SEMIBOLD 13, white on accent, padding [8, 18].
pub fn primary_btn(label: &str) -> button::Button<'_, Message> {
    button(text(label).font(FONT_SEMIBOLD).size(13).color(Color::WHITE))
        .style(accent_btn_style)
        .padding([8, 18])
}

/// Large primary — FONT_SEMIBOLD 14, white on accent, padding [10, 20].
pub fn primary_btn_lg(label: &str) -> button::Button<'_, Message> {
    button(text(label).font(FONT_SEMIBOLD).size(14).color(Color::WHITE))
        .style(accent_btn_style)
        .padding([10, 20])
}

/// Compact ghost — FONT_MEDIUM 12, custom color, transparent bg, padding [3, 8].
pub fn compact_ghost_btn(label: &str, color: Color) -> button::Button<'_, Message> {
    button(text(label).font(FONT_MEDIUM).size(12).color(color))
        .style(ghost_btn_style)
        .padding([3, 8])
}

/// Large ghost — FONT_MEDIUM 14, TEXT_MUTED, padding [10, 16].
pub fn ghost_btn_lg(label: &str) -> button::Button<'_, Message> {
    button(text(label).font(FONT_MEDIUM).size(14).color(TEXT_MUTED))
        .style(ghost_btn_style)
        .padding([10, 16])
}

/// Navigation arrow — FONT_MEDIUM 14, TEXT_MUTED, ghost bg, padding [4, 10].
pub fn nav_arrow_btn(glyph: &str) -> button::Button<'_, Message> {
    button(text(glyph).font(FONT_MEDIUM).size(14).color(TEXT_MUTED))
        .style(ghost_btn_style)
        .padding([4, 10])
}

/// Refresh button — FONT_MEDIUM 13, TEXT_MUTED, ghost bg, padding [4, 10].
pub fn refresh_btn(label: &str) -> button::Button<'_, Message> {
    button(text(label).font(FONT_MEDIUM).size(13).color(TEXT_MUTED))
        .style(ghost_btn_style)
        .padding([4, 10])
}

/// Outline button — FONT_MEDIUM 13, TEXT_MUTED, bordered, padding [9, 16].
pub fn outline_btn(label: &str) -> button::Button<'_, Message> {
    button(text(label).font(FONT_MEDIUM).size(13).color(TEXT_MUTED))
        .style(outline_btn_style)
        .padding([9, 16])
}

/// Compact outline button — FONT_MEDIUM 12, TEXT_MUTED, bordered, padding [8, 12].
pub fn outline_btn_sm(label: &str) -> button::Button<'_, Message> {
    button(text(label).font(FONT_MEDIUM).size(12).color(TEXT_MUTED))
        .style(outline_btn_style)
        .padding([8, 12])
}

/// Section heading — FONT_SEMIBOLD 14, TEXT_PRIMARY.
pub fn section_heading(title: &str) -> Element<'_, Message> {
    text(title).font(FONT_SEMIBOLD).size(14).color(TEXT_PRIMARY).into()
}

/// Suggestion dropdown container style — SURFACE bg, 8px radius, 1px border.
pub fn dropdown_container_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(crate::app::SURFACE)),
        border: Border {
            radius: 8.0.into(),
            color: Color { r: 0.165, g: 0.180, b: 0.231, a: 1.0 },
            width: 1.0,
        },
        ..Default::default()
    }
}

/// Card container style — SURFACE_RAISED bg, rounded, with subtle shadow.
pub fn card_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(SURFACE_RAISED)),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.20 },
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

/// Card style with an additional coloured border — used for highlighted form cards.
pub fn card_style_bordered(border_color: Color) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(SURFACE_RAISED)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: iced::Shadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.20 },
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

/// Accent-tinted chip container — ACCENT background and border at the given opacity levels.
pub fn accent_chip_style(bg_alpha: f32, border_alpha: f32) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(with_alpha(ACCENT, bg_alpha))),
        border: Border {
            color: with_alpha(ACCENT, border_alpha),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

/// Flat SURFACE strip with a subtle SURFACE_RAISED border — used for day-view
/// progress/work-day panels.
pub fn strip_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(SURFACE)),
        border: Border {
            color: SURFACE_RAISED,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// List row container style — SURFACE_RAISED bg, 8px radius, subtle shadow.
pub fn list_row_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(SURFACE_RAISED)),
        border: Border { radius: 8.0.into(), ..Default::default() },
        shadow: iced::Shadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.18 },
            offset: iced::Vector::new(0.0, 1.0),
            blur_radius: 4.0,
        },
        ..Default::default()
    }
}

// ── Shared widget builders ───────────────────────────────────────────────────

/// A compact stats chip with a coloured value, subtitle, and muted label.
pub fn stat_chip<'a>(
    label: &'a str,
    value: String,
    subtitle: String,
    color: Color,
) -> Element<'a, Message> {
    container(
        column![
            text(value).font(FONT_SEMIBOLD).size(20).color(color),
            text(subtitle).font(FONT_REGULAR).size(12).color(color),
            caption(label),
        ]
        .spacing(2)
        .align_x(Alignment::Center),
    )
    .style(|_| raised_container_style(8.0))
    .padding([10, 16])
    .width(Length::Fill)
    .into()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- format_hhmm ---

    #[test]
    fn format_hhmm_whole_hours() {
        assert_eq!(format_hhmm(0.0), "0:00");
        assert_eq!(format_hhmm(1.0), "1:00");
        assert_eq!(format_hhmm(8.0), "8:00");
    }

    #[test]
    fn format_hhmm_half_hours() {
        assert_eq!(format_hhmm(0.5), "0:30");
        assert_eq!(format_hhmm(2.5), "2:30");
    }

    #[test]
    fn format_hhmm_quarter_hours() {
        assert_eq!(format_hhmm(1.25), "1:15");
        assert_eq!(format_hhmm(1.75), "1:45");
    }

    #[test]
    fn format_hhmm_arbitrary_decimals() {
        // 7.75h = 7:45
        assert_eq!(format_hhmm(7.75), "7:45");
        // 0.1h = 6 minutes
        assert_eq!(format_hhmm(0.1), "0:06");
    }

    #[test]
    fn format_hhmm_large_values() {
        assert_eq!(format_hhmm(12.0), "12:00");
        assert_eq!(format_hhmm(100.5), "100:30");
    }

    // --- parse_hours ---

    #[test]
    fn parse_hours_decimal() {
        assert_eq!(parse_hours("2.5"), Some(2.5));
        assert_eq!(parse_hours("8"), Some(8.0));
        assert_eq!(parse_hours("0.25"), Some(0.25));
    }

    #[test]
    fn parse_hours_comma_decimal() {
        assert_eq!(parse_hours("2,5"), Some(2.5));
        assert_eq!(parse_hours("7,75"), Some(7.75));
    }

    #[test]
    fn parse_hours_hhmm_format() {
        assert_eq!(parse_hours("2:30"), Some(2.5));
        assert_eq!(parse_hours("1:15"), Some(1.25));
        assert_eq!(parse_hours("0:45"), Some(0.75));
        assert_eq!(parse_hours("8:00"), Some(8.0));
    }

    #[test]
    fn parse_hours_trims_whitespace() {
        assert_eq!(parse_hours("  2.5  "), Some(2.5));
        assert_eq!(parse_hours(" 1:30 "), Some(1.5));
    }

    #[test]
    fn parse_hours_rejects_zero_and_negative() {
        assert_eq!(parse_hours("0"), None);
        assert_eq!(parse_hours("0.0"), None);
        assert_eq!(parse_hours("-1"), None);
        assert_eq!(parse_hours("0:00"), None);
    }

    #[test]
    fn parse_hours_rejects_invalid_minutes() {
        assert_eq!(parse_hours("1:60"), None);
        assert_eq!(parse_hours("1:99"), None);
        assert_eq!(parse_hours("1:-5"), None);
    }

    #[test]
    fn parse_hours_rejects_garbage() {
        assert_eq!(parse_hours("abc"), None);
        assert_eq!(parse_hours(""), None);
        assert_eq!(parse_hours("::"), None);
        assert_eq!(parse_hours("1:2:3"), None);
    }

    #[test]
    fn parse_hours_roundtrip_with_format() {
        // format_hhmm(2.5) = "2:30", parse_hours("2:30") = 2.5
        let original = 2.5;
        let formatted = format_hhmm(original);
        let parsed = parse_hours(&formatted).unwrap();
        assert!((parsed - original).abs() < 1e-9);
    }
}
