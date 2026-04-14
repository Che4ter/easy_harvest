use iced::widget::{column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length};

use crate::app::{
    EasyHarvest, Message, StatsMsg, DANGER, FONT_REGULAR, FONT_SEMIBOLD, SUCCESS,
    SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{
    card_style, field_label, input_style, nav_arrow_btn, outline_btn_sm, primary_btn,
    refresh_btn, section_heading, PAGE_PADDING, SECTION_GAP,
};

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
        let adjustments_section = build_adjustments_section(state, year);

        scrollable(
            column![balance_card, adjustments_section]
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
        refresh_btn(refresh_label).on_press_maybe(
            if state.loading { None } else { Some(Message::Stats(StatsMsg::Refresh)) }
        ),
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

        let mut rows = vec![
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
        ];

        if bal.manual_adjustments_hours != 0.0 {
            let adj_sign = if bal.manual_adjustments_hours >= 0.0 { "+" } else { "" };
            rows.push(stat_row_owned(
                "Manual adjustments",
                format!("{adj_sign}{:.1}h", bal.manual_adjustments_hours),
                if bal.manual_adjustments_hours >= 0.0 { SUCCESS } else { DANGER },
            ));
        }

        rows.push(stat_row_owned(
            "Total balance",
            format!("{balance_sign}{balance:.1}h"),
            balance_color,
        ));

        stat_card(format!("{year} Overtime Balance"), rows)
    } else {
        empty_card("Overtime Balance", "No data yet")
    }
}

fn build_adjustments_section(state: &EasyHarvest, year: i32) -> Element<'_, Message> {
    let adjustments = state.overtime_adjustments.adjustments_for(year);

    let toggle_btn: Element<Message> = if state.overtime_adj_form.is_some() {
        Element::from(
            outline_btn_sm("Cancel")
                .on_press(Message::Stats(StatsMsg::HideAdjForm)),
        )
    } else {
        Element::from(
            outline_btn_sm("+ Add Adjustment")
                .on_press(Message::Stats(StatsMsg::ShowAdjForm)),
        )
    };

    let heading_row = row![
        section_heading("Adjustments"),
        Space::new().width(Length::Fill),
        toggle_btn,
    ]
    .align_y(Alignment::Center);

    let mut content = column![heading_row].spacing(8);

    // Form
    if let Some(form) = &state.overtime_adj_form {
        content = content.push(adjustment_form(form));
    }

    // List
    if adjustments.is_empty() {
        content = content.push(
            text("No adjustments for this year.")
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
        );
    } else {
        let rows: Vec<Element<Message>> = adjustments
            .iter()
            .map(|adj| adjustment_row(adj))
            .collect();
        content = content.push(column(rows).spacing(4));
    }

    container(content)
        .width(Length::Fill)
        .into()
}

fn adjustment_row(adj: &crate::state::overtime_adjustments::OvertimeAdjustment) -> Element<'_, Message> {
    let sign = if adj.hours >= 0.0 { "+" } else { "" };
    let color = if adj.hours >= 0.0 { SUCCESS } else { DANGER };

    // Format date for display: YYYY-MM-DD → DD.MM.YYYY
    let display_date = chrono::NaiveDate::parse_from_str(&adj.date, "%Y-%m-%d")
        .map(|d| d.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| adj.date.clone());

    let id = adj.id;
    container(
        row![
            text(display_date)
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED)
                .width(90),
            text(format!("{sign}{:.1}h", adj.hours))
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(color)
                .width(70),
            text(adj.reason.clone())
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_PRIMARY)
                .width(Length::Fill),
            outline_btn_sm("Delete")
                .on_press(Message::Stats(StatsMsg::AdjDelete(id))),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .style(card_style)
    .padding([8, 12])
    .width(Length::Fill)
    .into()
}

fn adjustment_form(form: &crate::app::OvertimeAdjustmentForm) -> Element<'_, Message> {
    let error_el: Element<Message> = if let Some(err) = &form.error {
        text(err.clone())
            .font(FONT_REGULAR)
            .size(12)
            .color(DANGER)
            .into()
    } else {
        Space::new().into()
    };

    container(
        column![
            field_label("Date (DD.MM.YYYY)"),
            text_input("DD.MM.YYYY", &form.date_input)
                .on_input(|v| Message::Stats(StatsMsg::AdjDateChanged(v)))
                .style(input_style)
                .size(13)
                .padding([8, 10]),
            field_label("Hours (negative to subtract)"),
            text_input("e.g. -8 or 4.5", &form.hours_input)
                .on_input(|v| Message::Stats(StatsMsg::AdjHoursChanged(v)))
                .style(input_style)
                .size(13)
                .padding([8, 10]),
            field_label("Reason"),
            text_input("e.g. Hours payout", &form.reason_input)
                .on_input(|v| Message::Stats(StatsMsg::AdjReasonChanged(v)))
                .style(input_style)
                .size(13)
                .padding([8, 10]),
            error_el,
            primary_btn("Add Adjustment")
                .on_press(Message::Stats(StatsMsg::AdjSubmit)),
        ]
        .spacing(8),
    )
    .style(card_style)
    .padding(14)
    .width(Length::Fill)
    .into()
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
