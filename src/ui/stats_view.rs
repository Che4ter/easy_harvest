use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::app::{
    EasyHarvest, Message, DANGER, FONT_REGULAR, FONT_SEMIBOLD, SUCCESS,
    SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{card_style, nav_arrow_btn, refresh_btn};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let year = state.overtime_year;

    let content: Element<Message> = if state.loading {
        container(
            text("Loading overtime…")
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
        )
        .center_x(Length::Fill)
        .padding([60, 0])
        .width(Length::Fill)
        .into()
    } else {
        let balance_card = build_balance_card(state, year);

        scrollable(
            column![balance_card]
                .spacing(16)
                .padding(Padding { top: 0.0, right: 0.0, bottom: 16.0, left: 0.0 }),
        )
        .height(Length::Fill)
        .into()
    };

    let year_nav = row![
        nav_arrow_btn("‹").on_press(Message::OvertimeYearPrev),
        text(format!("{year}"))
            .font(FONT_SEMIBOLD)
            .size(18)
            .color(TEXT_PRIMARY),
        nav_arrow_btn("›").on_press(Message::OvertimeYearNext),
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    let refresh_label = if state.loading { "Syncing…" } else { "↻  Refresh" };
    let heading = row![
        year_nav,
        Space::with_width(Length::Fill),
        refresh_btn(refresh_label).on_press(Message::StatsRefresh),
    ]
    .align_y(Alignment::Center);

    container(
        column![heading, Space::with_height(12), content].spacing(0),
    )
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn build_balance_card(state: &EasyHarvest, year: i32) -> Element<'_, Message> {
    if let Some(bal) = &state.year_balance {
        let balance = bal.total_balance;
        let balance_color = if balance >= 0.0 { SUCCESS } else { DANGER };
        let balance_sign = if balance >= 0.0 { "+" } else { "" };
        let period_sign = if bal.period.balance_hours >= 0.0 { "+" } else { "" };
        let carry_sign = if bal.carryover_hours >= 0.0 { "+" } else { "" };

        stat_card(
            format!("{year} Overtime Balance"),
            vec![
                stat_row_owned(
                    "Hours booked",
                    format!("{:.1}h", bal.period.total_hours),
                    TEXT_PRIMARY,
                ),
                stat_row_owned(
                    "Expected",
                    format!("{:.1}h", bal.period.expected_hours),
                    TEXT_MUTED,
                ),
                stat_row_owned(
                    "Period balance",
                    format!("{period_sign}{:.1}h", bal.period.balance_hours),
                    if bal.period.balance_hours >= 0.0 { SUCCESS } else { DANGER },
                ),
                divider(),
                stat_row_owned(
                    "Carryover from last year",
                    format!("{carry_sign}{:.1}h", bal.carryover_hours),
                    TEXT_MUTED,
                ),
                stat_row_owned(
                    "Total balance",
                    format!("{balance_sign}{balance:.1}h"),
                    balance_color,
                ),
            ],
        )
    } else {
        empty_card("Overtime Balance", "No data yet")
    }
}

fn stat_card(title: String, rows: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    container(
        column![
            text(title).font(FONT_SEMIBOLD).size(14).color(TEXT_PRIMARY),
            Space::with_height(16),
            column(rows).spacing(10),
        ]
        .spacing(0),
    )
    .style(card_style)
    .padding(14)
    .width(Length::Fill)
    .into()
}

fn empty_card<'a>(title: &'a str, hint: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title).font(FONT_SEMIBOLD).size(14).color(TEXT_MUTED),
            Space::with_height(16),
            text(hint).font(FONT_REGULAR).size(13).color(TEXT_MUTED),
        ]
        .spacing(0),
    )
    .style(card_style)
    .padding(14)
    .width(Length::Fill)
    .into()
}

fn stat_row_owned(
    label: &'static str,
    value: String,
    value_color: Color,
) -> Element<'static, Message> {
    row![
        text(label).font(FONT_REGULAR).size(14).color(TEXT_MUTED),
        Space::with_width(Length::Fill),
        text(value)
            .font(FONT_SEMIBOLD)
            .size(14)
            .color(value_color),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn divider() -> Element<'static, Message> {
    container(Space::with_height(1))
        .style(|_| container::Style {
            background: Some(iced::Background::Color(SURFACE_RAISED)),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(1)
        .into()
}
