use chrono::{Datelike, Local, NaiveDate};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::app::{
    EasyHarvest, Message, VacationForm, ACCENT, DANGER, FONT_MEDIUM, FONT_REGULAR,
    FONT_SEMIBOLD, SUCCESS, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
};
use crate::harvest::models::TimeEntry;
use super::{
    caption, delete_chip_btn, input_style, list_row_style, month_abbr, nav_arrow_btn,
    primary_btn, refresh_btn, stat_chip, toggle_active_style, toggle_inactive_style,
};

const PLANNED_COLOR: Color = Color { r: 0.42, g: 0.71, b: 0.98, a: 1.0 };

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let year = state.vacation.year;

    let add_label = if state.vacation.form.is_some() { "✕ Cancel" } else { "+ Add" };
    let add_msg = if state.vacation.form.is_some() {
        Message::VacationHideForm
    } else {
        Message::VacationShowForm
    };

    let year_row = row![
        nav_arrow_btn("‹").on_press(Message::VacationYearPrev),
        Space::with_width(10),
        text(year.to_string())
            .font(FONT_SEMIBOLD)
            .size(18)
            .color(TEXT_PRIMARY),
        Space::with_width(10),
        nav_arrow_btn("›").on_press(Message::VacationYearNext),
        Space::with_width(Length::Fill),
        refresh_btn("↻  Refresh").on_press(Message::VacationRefresh),
        Space::with_width(8),
        primary_btn(add_label).on_press(add_msg),
    ]
    .align_y(Alignment::Center);

    // Guard: no holiday tasks configured
    if state.settings.holiday_task_ids.is_empty() {
        return scrollable(
            column![
                year_row,
                Space::with_height(20),
                text("Configure holiday tasks in Settings → Holiday Tasks to track vacation.")
                    .font(FONT_REGULAR)
                    .size(13)
                    .color(TEXT_MUTED),
            ]
            .spacing(0)
            .padding(Padding { top: 12.0, right: 12.0, bottom: 0.0, left: 12.0 }),
        )
        .height(Length::Fill)
        .into();
    }

    if state.loading {
        return scrollable(
            column![
                year_row,
                Space::with_height(20),
                text("Loading…").font(FONT_REGULAR).size(13).color(TEXT_MUTED),
            ]
            .spacing(0)
            .padding(Padding { top: 12.0, right: 12.0, bottom: 0.0, left: 12.0 }),
        )
        .height(Length::Fill)
        .into();
    }

    let expected_per_day = state.settings.expected_hours_per_day();
    let task_ids = &state.settings.holiday_task_ids;
    let today = Local::now().naive_local().date();

    // Filter and sort vacation entries for this year
    let mut vac_entries: Vec<&TimeEntry> = state
        .vacation.entries
        .iter()
        .filter(|e| task_ids.contains(&e.task.id))
        .collect();
    vac_entries.sort_by(|a, b| a.spent_date.cmp(&b.spent_date));

    // Read from cached summary (fall back to zeros if not yet computed)
    let (used_days, booked_days, days_remaining, total_days, carryover_days) =
        if let Some(s) = &state.vacation.summary {
            (s.used_days, s.booked_days, s.days_remaining, s.total_days, s.carryover_days)
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0)
        };
    let rem_color = if days_remaining <= 5.0 { DANGER } else { SUCCESS };

    let summary_row = row![
        stat_chip("Used",      format!("{:.1}", used_days),      format!("({:.1}h)", used_days      * expected_per_day), ACCENT),
        Space::with_width(8),
        stat_chip("Planned",   format!("{:.1}", booked_days),    format!("({:.1}h)", booked_days    * expected_per_day), PLANNED_COLOR),
        Space::with_width(8),
        stat_chip("Remaining", format!("{:.1}", days_remaining), format!("({:.1}h)", days_remaining * expected_per_day), rem_color),
    ];

    // Add form
    let form_el: Element<Message> = match &state.vacation.form {
        Some(form) => vacation_form_view(form, expected_per_day),
        None => Space::with_height(0).into(),
    };

    // Entry list
    let body: Element<Message> = if vac_entries.is_empty() {
        text("No vacation entries for this year.")
            .font(FONT_REGULAR)
            .size(13)
            .color(TEXT_MUTED)
            .into()
    } else {
        let rows: Vec<Element<Message>> = vac_entries
            .iter()
            .map(|e| {
                let is_future = NaiveDate::parse_from_str(&e.spent_date, "%Y-%m-%d")
                    .map(|d| d > today)
                    .unwrap_or(false);
                vacation_row(e, expected_per_day, is_future)
            })
            .collect();
        column(rows).spacing(4).into()
    };

    scrollable(
        column![
            year_row,
            Space::with_height(14),
            summary_row,
            Space::with_height(4),
            {
                let carryover_str = if carryover_days != 0.0 {
                    format!(
                        "  ·  {}{:.1}d carryover ({:.1}h)",
                        if carryover_days > 0.0 { "+" } else { "" },
                        carryover_days,
                        carryover_days * expected_per_day
                    )
                } else {
                    String::new()
                };
                caption(format!(
                    "{:.1} days total entitlement  ({:.1}h){carryover_str}",
                    total_days,
                    total_days * expected_per_day
                ))
            },
            Space::with_height(14),
            form_el,
            body,
            Space::with_height(16),
        ]
        .spacing(0)
        .padding(Padding { top: 12.0, right: 12.0, bottom: 0.0, left: 12.0 }),
    )
    .height(Length::Fill)
    .into()
}

// ── Add vacation form ─────────────────────────────────────────────────────────

fn vacation_form_view(form: &VacationForm, expected_per_day: f64) -> Element<'_, Message> {
    let half_h = expected_per_day / 2.0;
    let full_h = expected_per_day;

    let full_label = format!("Full day  ({:.1}h)", full_h);
    let half_label = format!("Half day  ({:.1}h)", half_h);
    let full_day = form.full_day;

    let full_btn = button(text(full_label).font(FONT_MEDIUM).size(12))
        .style(move |_, _: button::Status| {
            if full_day { toggle_active_style(6.0) } else { toggle_inactive_style(6.0) }
        })
        .padding([5, 14])
        .on_press(Message::VacationDayTypeFull);
    let half_btn = button(text(half_label).font(FONT_MEDIUM).size(12))
        .style(move |_, _: button::Status| {
            if !full_day { toggle_active_style(6.0) } else { toggle_inactive_style(6.0) }
        })
        .padding([5, 14])
        .on_press(Message::VacationDayTypeHalf);

    let day_type_row = row![full_btn, Space::with_width(8), half_btn].align_y(Alignment::Center);

    let date_inputs = row![
        column![
            text("From").font(FONT_MEDIUM).size(12).color(TEXT_MUTED),
            text_input("DD.MM.YYYY", &form.from_input)
                .style(input_style)
                .size(13)
                .padding([8, 10])
                .on_input(Message::VacationFromChanged),
        ]
        .spacing(4)
        .width(Length::Fill),
        Space::with_width(10),
        column![
            text("To").font(FONT_MEDIUM).size(12).color(TEXT_MUTED),
            text_input("DD.MM.YYYY  (same as From = single day)", &form.to_input)
                .style(input_style)
                .size(13)
                .padding([8, 10])
                .on_input(Message::VacationToChanged),
        ]
        .spacing(4)
        .width(Length::FillPortion(2)),
    ]
    .align_y(Alignment::End);

    let hint = caption("Weekends and public holidays are skipped automatically.");

    let error_el: Element<Message> = if let Some(err) = &form.error {
        text(err.clone())
            .font(FONT_REGULAR)
            .size(12)
            .color(DANGER)
            .into()
    } else {
        Space::with_height(0).into()
    };

    let submit_label = if form.submitting { "Adding…" } else { "Add vacation" };
    let submit_btn = primary_btn(submit_label);
    let submit_btn: Element<Message> = if form.submitting {
        submit_btn.into()
    } else {
        submit_btn.on_press(Message::VacationFormSubmit).into()
    };

    let form_content = column![
        date_inputs,
        Space::with_height(10),
        day_type_row,
        Space::with_height(6),
        hint,
        error_el,
        Space::with_height(10),
        row![submit_btn].align_y(Alignment::Center),
    ]
    .spacing(0);

    let card = container(form_content)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(SURFACE_RAISED)),
            border: iced::Border {
                color: ACCENT,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: iced::Shadow {
                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.2 },
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            ..Default::default()
        })
        .padding([14, 14])
        .width(Length::Fill);

    column![card, Space::with_height(14)].spacing(0).into()
}

// ── Entry row ─────────────────────────────────────────────────────────────────

fn vacation_row(
    entry: &TimeEntry,
    expected_per_day: f64,
    is_future: bool,
) -> Element<'_, Message> {
    let date_str = NaiveDate::parse_from_str(&entry.spent_date, "%Y-%m-%d")
        .map(|d| format!("{:2} {} {}", d.day(), month_abbr(d.month()), d.year()))
        .unwrap_or_else(|_| entry.spent_date.clone());

    let days = entry.hours / expected_per_day;
    let day_str = if (days - 1.0).abs() < 0.02 {
        format!("1 day  ({:.1}h)", entry.hours)
    } else if (days - 0.5).abs() < 0.02 {
        format!("½ day  ({:.1}h)", entry.hours)
    } else {
        format!("{:.1} days  ({:.1}h)", days, entry.hours)
    };

    let hours_color = if is_future { PLANNED_COLOR } else { ACCENT };

    let notes = entry.notes.clone().unwrap_or_default();
    let notes_el: Element<'_, Message> = if !notes.is_empty() {
        caption(notes).into()
    } else {
        Space::with_height(0).into()
    };

    let delete_btn: Element<Message> = if is_future {
        let id = entry.id;
        delete_chip_btn(Message::VacationDeleteEntry(id))
    } else {
        Space::with_width(0).into()
    };

    container(
        row![
            text(date_str)
                .font(FONT_REGULAR)
                .size(12)
                .color(TEXT_MUTED)
                .width(96),
            column![
                text(entry.task.name.clone())
                    .font(FONT_MEDIUM)
                    .size(13)
                    .color(TEXT_PRIMARY),
                notes_el,
            ]
            .spacing(2)
            .width(Length::Fill),
            text(day_str)
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(hours_color),
            delete_btn,
        ]
        .align_y(Alignment::Center),
    )
    .style(list_row_style)
    .padding([10, 12])
    .width(Length::Fill)
    .into()
}


