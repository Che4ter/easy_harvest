use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Color, Element, Length};

use crate::app::{
    BillableMsg, EasyHarvest, Message, ACCENT, DANGER, FONT_MEDIUM, FONT_REGULAR, FONT_SEMIBOLD,
    SUCCESS, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{
    caption, format_hhmm, list_row_style, nav_arrow_btn, progress_bar, raised_container_style,
    refresh_btn, stat_chip, toggle_active_style, toggle_inactive_style,
    LIST_ROW_SPACING, PAGE_PADDING, SECTION_GAP,
};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let year = state.billable.year;
    let selected_month = state.billable.month;

    let year_row = row![
        nav_arrow_btn("‹").on_press(Message::Billable(BillableMsg::YearPrev)),
        Space::new().width(10).height(10),
        text(year.to_string())
            .font(FONT_SEMIBOLD)
            .size(18)
            .color(TEXT_PRIMARY),
        Space::new().width(10).height(10),
        nav_arrow_btn("›").on_press(Message::Billable(BillableMsg::YearNext)),
        Space::new().width(Length::Fill),
        refresh_btn("↻  Refresh").on_press(Message::Billable(BillableMsg::Refresh)),
    ]
    .align_y(Alignment::Center);

    // Month selector: "All" + Jan…Dec
    const MONTHS: [&str; 12] = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    let month_cells: Vec<Element<Message>> = std::iter::once({
        let active = selected_month.is_none();
        month_tab_btn("All", active, Message::Billable(BillableMsg::MonthClear))
    })
    .chain(MONTHS.iter().enumerate().map(|(i, &abbr)| {
        let m = (i + 1) as u32;
        let active = selected_month == Some(m);
        month_tab_btn(abbr, active, Message::Billable(BillableMsg::MonthSelected(m)))
    }))
    .collect();
    let month_row = row(month_cells).spacing(2);

    if state.loading {
        return scrollable(
            column![
                year_row,
                month_row,
                text("Loading…").font(FONT_REGULAR).size(13).color(TEXT_MUTED),
            ]
            .spacing(SECTION_GAP)
            .padding(PAGE_PADDING),
        )
        .height(Length::Fill)
        .into();
    }

    let entries = &state.billable.entries;

    let Some(summary) = &state.billable.summary else {
        return scrollable(
            column![
                year_row,
                month_row,
                text("No entries for this period.")
                    .font(FONT_REGULAR)
                    .size(13)
                    .color(TEXT_MUTED),
            ]
            .spacing(SECTION_GAP)
            .padding(PAGE_PADDING),
        )
        .height(Length::Fill)
        .into();
    };

    if entries.is_empty() {
        return scrollable(
            column![
                year_row,
                month_row,
                text("No entries for this period.")
                    .font(FONT_REGULAR)
                    .size(13)
                    .color(TEXT_MUTED),
            ]
            .spacing(SECTION_GAP)
            .padding(PAGE_PADDING),
        )
        .height(Length::Fill)
        .into();
    }

    // Read from cached summary
    let billable_hours = summary.billable_hours;
    let non_billable_hours = summary.non_billable_hours;
    let billable_pct = summary.billable_pct;
    let pct_color = if billable_pct >= 0.8 { SUCCESS } else if billable_pct >= 0.5 { ACCENT } else { DANGER };

    let summary_row = row![
        stat_chip("Billable",     format_hhmm(billable_hours),     format!("({:.1}h)", billable_hours),     ACCENT),
        Space::new().width(8).height(8),
        stat_chip("Non-billable", format_hhmm(non_billable_hours), format!("({:.1}h)", non_billable_hours), TEXT_MUTED),
        Space::new().width(8).height(8),
        pct_chip(billable_pct, pct_color),
    ];

    let rows: Vec<Element<Message>> = summary.projects
        .iter()
        .map(|(name, client, b, t)| project_row(name, client, *b, *t))
        .collect();

    scrollable(
        column![
            year_row,
            month_row,
            summary_row,
            progress_bar(billable_pct as f32, pct_color, 6),
            caption(format!("{:.1}% of all hours are billable", billable_pct * 100.0)),
            column(rows).spacing(LIST_ROW_SPACING),
        ]
        .spacing(SECTION_GAP)
        .padding(PAGE_PADDING),
    )
    .height(Length::Fill)
    .into()
}

fn project_row<'a>(name: &str, client: &str, billable_h: f64, total_h: f64) -> Element<'a, Message> {
    let pct = if total_h > 0.0 { billable_h / total_h } else { 0.0 };
    let bar_color = if pct >= 0.8 { SUCCESS } else if pct >= 0.5 { ACCENT } else { DANGER };
    let pct_color = bar_color;

    container(
        column![
            row![
                column![
                    text(name.to_owned())
                        .font(FONT_MEDIUM)
                        .size(13)
                        .color(TEXT_PRIMARY),
                    caption(client.to_owned()),
                ]
                .spacing(2)
                .width(Length::Fill),
                column![
                    text(format!("{} / {}", format_hhmm(billable_h), format_hhmm(total_h)))
                        .font(FONT_SEMIBOLD)
                        .size(13)
                        .color(ACCENT),
                    text(format!("{:.0}% billable", pct * 100.0))
                        .font(FONT_REGULAR)
                        .size(12)
                        .color(pct_color),
                ]
                .spacing(2)
                .align_x(Alignment::End),
            ]
            .align_y(Alignment::Center),
            progress_bar(pct as f32, bar_color, 4),
        ]
        .spacing(6),
    )
    .style(list_row_style)
    .padding([10, 12])
    .width(Length::Fill)
    .into()
}


fn pct_chip(pct: f64, color: Color) -> Element<'static, Message> {
    container(
        column![
            text(format!("{:.1}%", pct * 100.0)).font(FONT_SEMIBOLD).size(20).color(color),
            text("").font(FONT_REGULAR).size(12),
            caption("billable"),
        ]
        .spacing(2)
        .align_x(Alignment::Center),
    )
    .style(|_| raised_container_style(8.0))
    .padding([10, 16])
    .width(Length::Fill)
    .into()
}

fn month_tab_btn(label: &str, active: bool, msg: Message) -> Element<'static, Message> {
    button(
        text(label.to_owned())
            .font(FONT_MEDIUM)
            .size(12)
            .align_x(Alignment::Center),
    )
    .style(move |_, _: iced::widget::button::Status| {
        if active {
            toggle_active_style(5.0)
        } else {
            toggle_inactive_style(5.0)
        }
    })
    .padding([4, 6])
    .width(Length::FillPortion(1))
    .on_press(msg)
    .into()
}
