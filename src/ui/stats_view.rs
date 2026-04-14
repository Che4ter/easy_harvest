use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Color, Element, Length};

use crate::app::{
    EasyHarvest, Message, StatsMsg, DANGER, FONT_REGULAR, FONT_SEMIBOLD, SUCCESS,
    SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{card_style, nav_arrow_btn, refresh_btn, PAGE_PADDING, SECTION_GAP};

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
                .spacing(SECTION_GAP),
        )
        .height(Length::Fill)
        .into()
    };

    let year_nav = row![
        nav_arrow_btn("‹").on_press(Message::Stats(StatsMsg::YearPrev)),
        Space::new().width(10).height(10),
        text(year.to_string())
            .font(FONT_SEMIBOLD)
            .size(18)
            .color(TEXT_PRIMARY),
        Space::new().width(10).height(10),
        nav_arrow_btn("›").on_press(Message::Stats(StatsMsg::YearNext)),
    ]
    .align_y(Alignment::Center);

    let refresh_label = if state.loading { "Syncing…" } else { "↻  Refresh" };
    let heading = row![
        year_nav,
        Space::new().width(Length::Fill),
        refresh_btn(refresh_label).on_press(Message::Stats(StatsMsg::Refresh)),
    ]
    .align_y(Alignment::Center);

    container(
        column![heading, content].spacing(SECTION_GAP),
    )
    .padding(PAGE_PADDING)
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
            column(rows).spacing(10),
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

fn empty_card<'a>(title: &'a str, hint: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title).font(FONT_SEMIBOLD).size(14).color(TEXT_MUTED),
            text(hint).font(FONT_REGULAR).size(13).color(TEXT_MUTED),
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
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
        Space::new().width(Length::Fill),
        text(value)
            .font(FONT_SEMIBOLD)
            .size(14)
            .color(value_color),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn divider() -> Element<'static, Message> {
    container(Space::new())
        .style(|_| container::Style {
            background: Some(iced::Background::Color(SURFACE_RAISED)),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(1)
        .into()
}
