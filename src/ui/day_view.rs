use chrono::{Datelike, Local, NaiveDate, Weekday};
use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Space,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::app::{
    EasyHarvest, EntryMsg, Message, NavMsg, WorkDayMsg, ACCENT, DANGER, FONT_MEDIUM, FONT_REGULAR,
    FONT_SEMIBOLD, SUCCESS, SURFACE, SURFACE_HOVER, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, WARNING,
};
use crate::harvest::models::TimeEntry;
use crate::state::favorites::ProjectOption;
use crate::state::work_day::WorkPhase;

use super::{
    caption, compact_ghost_btn, danger_btn_style, dropdown_container_style,
    field_label, ghost_btn_lg, list_row_style, month_name,
    nav_arrow_btn, primary_btn_lg, progress_bar_with_marker, section_heading,
    suggestion_btn_style, with_alpha, LIST_ROW_SPACING,
};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let header = date_header(state);
    let hours_bar = hours_summary(state);
    let work_strip = work_day_strip(state);
    let body: Element<Message> = if state.entry_form.is_some() {
        entry_form_view(state)
    } else {
        entry_list(state)
    };

    let mut col = column![header, hours_bar, work_strip]
        .spacing(0)
        .width(Length::Fill);

    if state.date_picker.open {
        col = col.push(date_picker_view(state));
    }

    col.push(body).into()
}

// ── Date header ───────────────────────────────────────────────────────────────

fn date_header(state: &EasyHarvest) -> Element<'_, Message> {
    let today = Local::now().naive_local().date();
    let d = state.current_date;

    let weekday = match d.weekday() {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    };
    let date_str = format!("{weekday}, {} {}", month_name(d.month()), d.day());
    let is_today = d == today;

    let today_link: Element<'_, Message> = if is_today {
        text("Today").font(FONT_REGULAR).size(12).color(ACCENT).into()
    } else {
        button(
            text("→ Today").font(FONT_REGULAR).size(12).color(TEXT_MUTED),
        )
        .style(super::plain_btn_style)
        .padding([0, 0])
        .on_press(Message::Nav(NavMsg::DateToday))
        .into()
    };

    let date_btn = button(
        text(date_str).font(FONT_SEMIBOLD).size(18).color(TEXT_PRIMARY),
    )
    .style(super::plain_btn_style)
    .padding([0, 0])
    .on_press(Message::Nav(NavMsg::DatePickerToggle));

    let date_label: Element<Message> = column![date_btn, today_link]
        .spacing(1)
        .into();

    let prev_btn: Element<Message> = nav_arrow_btn("‹").on_press(Message::Nav(NavMsg::DatePrev)).into();
    let next_btn: Element<Message> = nav_arrow_btn("›").on_press(Message::Nav(NavMsg::DateNext)).into();

    let lock_badge: Element<Message> = day_lock_badge(&state.entries);

    container(
        row![
            prev_btn,
            Space::new().width(8).height(8),
            date_label,
            Space::new().width(Length::Fill),
            lock_badge,
            Space::new().width(8).height(8),
            next_btn,
        ]
        .align_y(Alignment::Center),
    )
    .style(|_| container::Style {
        background: Some(iced::Background::Color(SURFACE)),
        ..Default::default()
    })
    .padding([8, 12])
    .width(Length::Fill)
    .into()
}

/// Returns a small "Locked" or "Billed" pill if all entries for the day are locked.
fn day_lock_badge(entries: &[crate::harvest::models::TimeEntry]) -> Element<'static, Message> {
    if entries.is_empty() {
        return Space::new().into();
    }
    let all_locked = entries.iter().all(|e| e.is_locked);
    if !all_locked {
        return Space::new().into();
    }
    let all_billed = entries.iter().all(|e| e.is_billed);
    let (label, color) = if all_billed {
        ("Billed", TEXT_MUTED)
    } else {
        ("Locked", ACCENT)
    };
    container(
        text(label).font(FONT_MEDIUM).size(12).color(color),
    )
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(Color {
            r: color.r,
            g: color.g,
            b: color.b,
            a: 0.15,
        })),
        border: iced::Border {
            color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .padding([3, 8])
    .into()
}

// ── Hours summary bar ─────────────────────────────────────────────────────────

fn hours_summary(state: &EasyHarvest) -> Element<'_, Message> {
    let booked: f64 = state.entries.iter().map(|e| e.hours).sum();
    let expected = state.cached_expected_hours;

    // Drive the bar off the work day tracker so it reflects actual time at desk.
    let now = Local::now().naive_local();
    let work_day = state.work_day_store.get_or_default(state.current_date);
    let worked_h = work_day.worked_hours(now.time());
    let phase = work_day.phase();

    // How many worked hours have not yet been entered in Harvest.
    let unbooked_worked = (worked_h - booked).max(0.0);
    // How much is booked beyond what was actually worked locally.
    let overtime = (booked - worked_h).max(0.0);
    // Bar target: you need to book at least `expected`, or all worked time if longer.
    let target = worked_h.max(expected);

    let pct = if target > 0.0 {
        (booked / target).min(1.0) as f32
    } else {
        1.0
    };

    // Red → amber → green as booking progresses.
    let bar_color = if pct >= 1.0 {
        SUCCESS
    } else if pct >= 0.9 {
        WARNING
    } else {
        DANGER
    };

    // Tick on the bar at the expected-hours threshold so the user can see
    // where their daily target sits relative to total worked time.
    let marker_pct = if target > 0.0 { (expected / target).min(1.0) as f32 } else { 1.0 };

    // Left label: how long you've actually been working.
    let worked_label = if phase == WorkPhase::NotStarted {
        text("Not started".to_string())
            .font(FONT_REGULAR).size(13).color(TEXT_MUTED)
    } else {
        text(format!("{} worked", super::fmt_hm(worked_h)))
            .font(FONT_MEDIUM).size(13).color(TEXT_PRIMARY)
    };

    // Right label: purely compares booked against worked time.
    //  · booked < worked  → red   "X:XX to book"      (under-booked)
    //  · booked ≈ worked  → green "All hours booked"   (exact match)
    //  · booked > worked  → amber "X:XX overbooked"    (over-booked)
    // When the day hasn't started yet and nothing is booked, stay silent.
    let right_label = if phase == WorkPhase::NotStarted && booked < 0.01 {
        text(String::new()).size(12)
    } else if unbooked_worked > 0.01 {
        text(format!("{} to book", super::fmt_hm(unbooked_worked)))
            .font(FONT_MEDIUM).size(12).color(DANGER)
    } else if overtime > 0.01 {
        text(format!("{} overbooked", super::fmt_hm(overtime)))
            .font(FONT_MEDIUM).size(12).color(WARNING)
    } else {
        text("All hours booked".to_string())
            .font(FONT_MEDIUM).size(12).color(SUCCESS)
    };

    let labels = row![
        worked_label,
        Space::new().width(Length::Fill),
        right_label,
    ]
    .align_y(Alignment::Center);

    container(
        column![labels, progress_bar_with_marker(pct, bar_color, marker_pct, 6)].spacing(6),
    )
    .style(super::strip_style)
    .padding([8, 12])
    .width(Length::Fill)
    .into()
}

// ── Work-day strip ────────────────────────────────────────────────────────────

fn work_day_strip(state: &EasyHarvest) -> Element<'_, Message> {
    let now = Local::now().naive_local();
    let today = now.date();
    let is_today = state.current_date == today;
    let work_day = state.work_day_store.get_or_default(state.current_date);
    let phase = work_day.phase();

    if !is_today && phase == WorkPhase::NotStarted {
        return Space::new().into();
    }

    let (phase_label, phase_color) = match phase {
        WorkPhase::NotStarted => ("Not started", TEXT_MUTED),
        WorkPhase::Working    => ("Working",     SUCCESS),
        WorkPhase::OnBreak    => ("On break",    ACCENT),
        WorkPhase::Ended      => ("Done",        TEXT_MUTED),
    };

    let dot: Element<Message> = container(Space::new())
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(phase_color)),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            ..Default::default()
        })
        .width(8).height(8)
        .into();

    // Time label removed — the hours_summary bar above already shows the worked time.

    // ── Build the strip content depending on edit mode ────────────────────────

    // Edit mode works for both today and past days.
    let in_edit = state.work_day_edit.edit_mode;

    // Right-side controls for the status row
    let controls: Element<Message> = if in_edit {
        row![
            wd_secondary_btn("Cancel", Message::WorkDay(WorkDayMsg::EditCancel)),
            Space::new().width(6).height(6),
            wd_primary_btn("Save", SUCCESS, Message::WorkDay(WorkDayMsg::EditSave)),
        ]
        .align_y(Alignment::Center)
        .into()
    } else if is_today {
        match phase {
            WorkPhase::NotStarted => wd_primary_btn("Start Day", SUCCESS, Message::WorkDay(WorkDayMsg::Start)),
            WorkPhase::Working => row![
                wd_secondary_btn("Start Break", Message::WorkDay(WorkDayMsg::StartBreak)),
                Space::new().width(6).height(6),
                wd_primary_btn("End Day", DANGER, Message::WorkDay(WorkDayMsg::End)),
            ]
            .align_y(Alignment::Center)
            .into(),
            WorkPhase::OnBreak => wd_primary_btn("End Break", ACCENT, Message::WorkDay(WorkDayMsg::EndBreak)),
            WorkPhase::Ended => row![
                wd_secondary_btn("Resume Day", Message::WorkDay(WorkDayMsg::Resume)),
            ]
            .align_y(Alignment::Center)
            .into(),
        }
    } else {
        Space::new().into()
    };

    // Timeline summary shown in display mode
    let timeline_summary: Element<Message> = match work_day.start_time {
        None => Space::new().into(),
        Some(start) => {
            let end_part = match work_day.end_time {
                Some(e) => e.format("%H:%M").to_string(),
                None    => "…".into(),
            };
            let break_mins = work_day.break_duration().num_minutes();
            let break_part = if break_mins > 0 {
                format!("  ·  {}:{:02} break", break_mins / 60, break_mins % 60)
            } else {
                String::new()
            };
            text(format!("{} → {}{}", start.format("%H:%M"), end_part, break_part))
                .font(FONT_REGULAR).size(12).color(TEXT_MUTED)
                .into()
        }
    };

    // "Edit" button — shown in display mode for any day (today or past) that has data.
    let edit_btn: Element<Message> = if !in_edit && phase != WorkPhase::NotStarted {
        button(text("Edit").font(FONT_MEDIUM).size(12).color(TEXT_MUTED))
            .on_press(Message::WorkDay(WorkDayMsg::EditStart))
            .padding([1, 6])
            .style(|_: &iced::Theme, status| button::Style {
                background: Some(iced::Background::Color(match status {
                    button::Status::Hovered => SURFACE_HOVER,
                    _ => SURFACE_RAISED,
                })),
                text_color: TEXT_MUTED,
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
    } else {
        Space::new().into()
    };

    // Status row: dot + phase + [timeline] + [Edit] + controls
    let status_row: Element<Message> = row![
        dot,
        Space::new().width(7).height(7),
        text(phase_label).font(FONT_MEDIUM).size(12).color(phase_color),
        Space::new().width(Length::Fill),
        if in_edit { Space::new().into() } else { timeline_summary },
        Space::new().width(6).height(6),
        edit_btn,
        Space::new().width(10).height(10),
        controls,
    ]
    .align_y(Alignment::Center)
    .into();

    // ── Edit panel (only visible in edit mode) ────────────────────────────────

    let edit_panel: Element<Message> = if in_edit {
        work_day_edit_panel(state)
    } else {
        Space::new().into()
    };

    container(
        column![status_row, edit_panel].spacing(0),
    )
    .style(super::strip_style)
    .padding([8, 12])
    .width(Length::Fill)
    .into()
}

fn work_day_edit_panel(state: &EasyHarvest) -> Element<'_, Message> {
    let label_col = |s: &str| -> Element<Message> {
        text(s.to_owned()).font(FONT_REGULAR).size(12).color(TEXT_MUTED).width(46).into()
    };

    let arrow = || -> Element<'static, Message> {
        text("→").font(FONT_REGULAR).size(12).color(TEXT_MUTED).into()
    };

    // Start row
    let start_row: Element<Message> = row![
        label_col("Start"),
        text_input("HH:MM", &state.work_day_edit.start_input)
            .on_input(|v| Message::WorkDay(WorkDayMsg::StartInputChanged(v)))
            .size(12).padding([3, 6]).width(58).style(super::input_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into();

    // Break rows
    let mut rows: Vec<Element<Message>> = vec![start_row];

    for (idx, (b_start, b_end)) in state.work_day_edit.break_inputs.iter().enumerate() {
        let label = format!("Break {}", idx + 1);
        let del_btn = super::delete_chip_btn(Message::WorkDay(WorkDayMsg::BreakDelete(idx)));

        let bs = b_start.clone();
        let be = b_end.clone();
        let row_el: Element<Message> = row![
            label_col(&label),
            text_input("HH:MM", &bs)
                .on_input(move |v| Message::WorkDay(WorkDayMsg::BreakStartChanged(idx, v)))
                .size(12).padding([3, 6]).width(58).style(super::input_style),
            arrow(),
            text_input("HH:MM", &be)
                .on_input(move |v| Message::WorkDay(WorkDayMsg::BreakEndChanged(idx, v)))
                .size(12).padding([3, 6]).width(58).style(super::input_style),
            del_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into();
        rows.push(row_el);
    }

    // End row
    let end_row: Element<Message> = row![
        label_col("End"),
        text_input("HH:MM", &state.work_day_edit.end_input)
            .on_input(|v| Message::WorkDay(WorkDayMsg::EndInputChanged(v)))
            .size(12).padding([3, 6]).width(58).style(super::input_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into();
    rows.push(end_row);

    // + Add Break button
    let add_break_btn: Element<Message> = button(
        text("+ Add Break").font(FONT_MEDIUM).size(12).color(ACCENT),
    )
    .on_press(Message::WorkDay(WorkDayMsg::BreakAdd))
    .padding([3, 8])
    .style(|_, status| button::Style {
        background: Some(iced::Background::Color(match status {
            button::Status::Hovered => with_alpha(ACCENT, 0.1),
            _ => Color::TRANSPARENT,
        })),
        text_color: ACCENT,
        border: iced::Border { color: with_alpha(ACCENT, 0.4), width: 1.0, radius: 4.0.into() },
        ..Default::default()
    })
    .into();
    rows.push(add_break_btn);

    column![
        Space::new().height(6),
        column(rows).spacing(5),
    ]
    .spacing(0)
    .into()
}

fn wd_primary_btn(
    label: &'static str,
    color: Color,
    msg: Message,
) -> Element<'static, Message> {
    button(text(label).font(FONT_SEMIBOLD).size(12).color(Color::WHITE))
        .style(move |_, status| {
            let alpha = match status {
                button::Status::Hovered  => 0.85,
                button::Status::Pressed  => 0.70,
                _                        => 1.0,
            };
            button::Style {
                background: Some(iced::Background::Color(Color { a: alpha, ..color })),
                text_color: Color::WHITE,
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }
        })
        .padding([5, 12])
        .on_press(msg)
        .into()
}

fn wd_secondary_btn(label: &'static str, msg: Message) -> Element<'static, Message> {
    button(text(label).font(FONT_MEDIUM).size(12).color(TEXT_PRIMARY))
        .style(|_, status| {
            let bg = match status {
                button::Status::Hovered => SURFACE_HOVER,
                _                       => SURFACE_RAISED,
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: TEXT_PRIMARY,
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }
        })
        .padding([5, 10])
        .on_press(msg)
        .into()
}

// ── Entry list ────────────────────────────────────────────────────────────────

fn entry_list(state: &EasyHarvest) -> Element<'_, Message> {
    let add_btn = button(
        row![
            text("+").font(FONT_SEMIBOLD).size(16).color(Color::WHITE),
            Space::new().width(6).height(6),
            text("Add Entry").font(FONT_MEDIUM).size(14).color(Color::WHITE),
        ]
        .align_y(Alignment::Center),
    )
    .style(super::accent_btn_style)
    .padding([8, 18])
    .on_press(Message::Entry(Box::new(EntryMsg::ShowForm)));

    let header = container(
        row![
            section_heading("Time Entries"),
            Space::new().width(Length::Fill),
            add_btn,
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::ZERO.bottom(12))
    .width(Length::Fill);

    let entries: Element<Message> = if state.loading {
        container(
            text("Loading…")
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
        )
        .padding([32, 0])
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
    } else if state.entries.is_empty() {
        container(
            column![
                text("No entries yet")
                    .font(FONT_MEDIUM)
                    .size(14)
                    .color(TEXT_MUTED),
                text("Click + Add Entry to start tracking")
                    .font(FONT_REGULAR)
                    .size(12)
                    .color(super::with_alpha(TEXT_MUTED, 0.6)),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .padding([40, 0])
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
    } else {
        let pending = state.pending_delete;
        let rows: Vec<Element<Message>> = state
            .entries
            .iter()
            .map(|e| entry_row(e, pending))
            .collect();
        scrollable(
            column(rows).spacing(LIST_ROW_SPACING).width(Length::Fill).padding([0, 2]),
        )
        .height(Length::Fill)
        .into()
    };

    container(column![header, entries].spacing(0))
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn entry_row(entry: &TimeEntry, pending_delete: Option<i64>) -> Element<'_, Message> {
    let locked_indicator: Element<Message> = if entry.is_billed {
        status_badge("Billed", TEXT_MUTED)
    } else if entry.approval_status.as_deref() == Some("submitted") {
        status_badge("Pending", ACCENT)
    } else if entry.approval_status.as_deref() == Some("approved") {
        status_badge("Approved", SUCCESS)
    } else if entry.is_locked {
        status_badge("Locked", ACCENT)
    } else {
        Space::new().into()
    };

    let running_dot: Element<Message> = if entry.is_running {
        container(Space::new().width(8).height(8))
            .style(|_| container::Style {
                background: Some(iced::Background::Color(SUCCESS)),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .width(8).height(8)
            .into()
    } else {
        Space::new().into()
    };

    let client_project = text(format!("{} › {}", entry.client.name, entry.project.name))
        .font(FONT_MEDIUM).size(13).color(TEXT_PRIMARY);

    let task = text(&entry.task.name).font(FONT_REGULAR).size(12).color(TEXT_MUTED);

    let notes: Element<Message> = match entry.notes.as_deref() {
        Some(n) if !n.is_empty() => {
            text(n).font(FONT_REGULAR).size(12).color(TEXT_MUTED).into()
        }
        _ => Space::new().into(),
    };

    let hours = text(super::format_hhmm(entry.hours))
        .font(FONT_SEMIBOLD).size(13).color(ACCENT);

    let timer_btn: Element<Message> = if entry.is_running {
        compact_ghost_btn("■  Stop", SUCCESS)
            .on_press(Message::Entry(Box::new(EntryMsg::TimerStop(entry.id))))
            .into()
    } else if !entry.is_locked && entry.approval_status.as_deref() != Some("submitted") {
        compact_ghost_btn("▶  Start", TEXT_MUTED)
            .on_press(Message::Entry(Box::new(EntryMsg::TimerStart(entry.id))))
            .into()
    } else {
        Space::new().into()
    };

    // Action area — confirmation state or normal edit/delete buttons
    let locked_or_pending = entry.is_locked
        || entry.approval_status.as_deref() == Some("submitted");
    let actions: Element<Message> = if locked_or_pending {
        Space::new().into()
    } else if pending_delete == Some(entry.id) {
        // Confirmation row
        row![
            text("Delete?").font(FONT_MEDIUM).size(12).color(DANGER),
            Space::new().width(6).height(6),
            compact_ghost_btn("Cancel", TEXT_MUTED)
                .on_press(Message::Entry(Box::new(EntryMsg::DeleteCancel))),
            Space::new().width(4).height(4),
            button(text("Confirm").font(FONT_SEMIBOLD).size(12).color(Color::WHITE))
                .style(danger_btn_style)
                .padding([3, 8])
                .on_press(Message::Entry(Box::new(EntryMsg::Delete(entry.id)))),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            timer_btn,
            Space::new().width(2).height(2),
            compact_ghost_btn("Edit", TEXT_MUTED)
                .on_press(Message::Entry(Box::new(EntryMsg::Edit(entry.id)))),
            Space::new().width(2).height(2),
            compact_ghost_btn("Delete", DANGER)
                .on_press(Message::Entry(Box::new(EntryMsg::DeleteRequest(entry.id)))),
        ]
        .align_y(Alignment::Center)
        .into()
    };

    let left_col = column![
        row![running_dot, Space::new().width(4).height(4), client_project]
            .align_y(Alignment::Center),
        task,
        notes,
    ]
    .spacing(2)
    .width(Length::Fill);

    let right_col = column![
        locked_indicator,
        hours,
        actions,
    ]
    .spacing(4)
    .align_x(Alignment::End);

    container(
        row![left_col, right_col].align_y(Alignment::Start),
    )
    .style(list_row_style)
    .padding([10, 12])
    .width(Length::Fill)
    .into()
}

// ── Entry form ────────────────────────────────────────────────────────────────

fn entry_form_view(state: &EasyHarvest) -> Element<'_, Message> {
    let Some(form) = state.entry_form.as_ref() else {
        return Space::new().into();
    };
    let options = &state.cached_project_options;

    let title = if form.editing_id.is_some() {
        "Edit Entry"
    } else {
        "New Entry"
    };

    let heading = text(title)
        .font(FONT_SEMIBOLD)
        .size(16)
        .color(TEXT_PRIMARY);

    // Template quick-select chips (only for new entries)
    let templates_section: Element<Message> = if form.editing_id.is_none()
        && !state.templates.entries.is_empty()
    {
        let chips: Vec<Element<Message>> = state
            .templates
            .entries
            .iter()
            .enumerate()
            .map(|(idx, tpl)| {
                let label = tpl.label.clone();
                button(text(label).font(FONT_MEDIUM).size(12).color(TEXT_PRIMARY))
                    .style(|_, status| button::Style {
                        background: Some(iced::Background::Color(match status {
                            button::Status::Hovered => with_alpha(ACCENT, 0.1),
                            _ => SURFACE_RAISED,
                        })),
                        text_color: TEXT_PRIMARY,
                        border: iced::Border {
                            color: with_alpha(ACCENT, 0.3),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    })
                    .padding([5, 10])
                    .on_press(Message::Entry(Box::new(EntryMsg::TemplateApply(idx))))
                    .into()
            })
            .collect();

        column![
            caption("Quick fill from template"),
            scrollable(row(chips).spacing(6))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(2).scroller_width(2),
                ))
                .width(Length::Fill),
        ]
        .spacing(4)
        .into()
    } else {
        Space::new().into()
    };

    // Project search
    let project_label = field_label("Project & Task");
    let project_input = {
        let inp = text_input("Search project or task…", &form.project_query)
            .on_input(|v| Message::Entry(Box::new(EntryMsg::ProjectQueryChanged(v))))
            .size(14)
            .padding([10, 12])
            .style(super::input_style);
        // Advance focus to hours when project is already confirmed
        if form.selected_project_idx.is_some() {
            inp.on_submit(Message::Entry(Box::new(EntryMsg::FocusHours)))
        } else {
            inp
        }
    };

    // Filtered suggestions
    let query = form.project_query.to_lowercase();
    let suggestions: Vec<(usize, &ProjectOption)> = options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.matches_query(&query))
        .take(6)
        .collect();

    let suggestion_list: Element<Message> = if !suggestions.is_empty()
        && form.selected_project_idx.is_none()
        && !form.project_query.is_empty()
    {
        let items: Vec<Element<Message>> = suggestions
            .iter()
            .map(|(idx, opt)| {
                let idx = *idx;
                let pin = if opt.is_pinned { "★ " } else { "" };
                button(
                    text(format!("{pin}{}", opt.search_text))
                        .font(FONT_REGULAR)
                        .size(13)
                        .color(TEXT_PRIMARY),
                )
                .style(suggestion_btn_style)
                .padding([8, 12])
                .width(Length::Fill)
                .on_press(Message::Entry(Box::new(EntryMsg::ProjectSelected(idx))))
                .into()
            })
            .collect();

        container(column(items).spacing(1))
            .style(dropdown_container_style)
            .width(Length::Fill)
            .into()
    } else {
        Space::new().into()
    };

    // Hours
    // Compute how much time is still unaccounted for so we can offer a quick-fill.
    let booked_total: f64 = state.entries.iter().map(|e| e.hours).sum();
    let editing_hours = form
        .editing_id
        .and_then(|id| state.entries.iter().find(|e| e.id == id))
        .map(|e| e.hours)
        .unwrap_or(0.0);
    let worked_h_now = state
        .work_day_store
        .get_or_default(state.current_date)
        .worked_hours(Local::now().naive_local().time());
    let fill_target = worked_h_now.max(state.cached_expected_hours);
    let fill_remaining = (fill_target - (booked_total - editing_hours)).max(0.0);

    let hours_label: Element<Message> = if fill_remaining > 0.01 {
        {
            let fill_btn: Element<Message> = button(
                text(format!("Fill {}", super::fmt_hm(fill_remaining)))
                    .font(FONT_MEDIUM).size(11).color(ACCENT),
            )
            .on_press(Message::Entry(Box::new(EntryMsg::FillRemaining)))
            .padding([2, 8])
            .style(|_: &iced::Theme, status| button::Style {
                background: Some(iced::Background::Color(match status {
                    button::Status::Hovered => with_alpha(ACCENT, 0.15),
                    _ => with_alpha(ACCENT, 0.07),
                })),
                border: iced::Border {
                    color: with_alpha(ACCENT, 0.3),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into();
            row![field_label("Hours"), Space::new().width(Length::Fill), fill_btn]
                .align_y(Alignment::Center)
                .into()
        }
    } else {
        field_label("Hours")
    };
    let hours_input = text_input("e.g. 2:30", &form.hours_input)
        .id(iced::widget::Id::new("form_hours"))
        .on_input(|v| Message::Entry(Box::new(EntryMsg::HoursChanged(v))))
        .on_submit(Message::Entry(Box::new(EntryMsg::FocusNotes)))
        .size(14)
        .padding([10, 12])
        .style(super::input_style);

    // Notes
    let notes_label = field_label("Notes (optional)");
    let notes_input = text_input("What did you work on?", &form.notes_input)
        .id(iced::widget::Id::new("form_notes"))
        .on_input(|v| Message::Entry(Box::new(EntryMsg::NotesChanged(v))))
        .on_submit(Message::Entry(Box::new(EntryMsg::Submit)))
        .size(14)
        .padding([10, 12])
        .style(super::input_style);

    // Error
    let error: Element<Message> = if let Some(err) = &form.error {
        text(err.as_str())
            .font(FONT_REGULAR)
            .size(12)
            .color(DANGER)
            .into()
    } else {
        Space::new().into()
    };

    // Buttons
    let submit_label = if form.editing_id.is_some() {
        "Save Changes"
    } else {
        "Add Entry"
    };
    let submit_btn = primary_btn_lg(submit_label)
        .on_press(Message::Entry(Box::new(EntryMsg::Submit)));

    let cancel_btn = ghost_btn_lg("Cancel")
        .on_press(Message::Entry(Box::new(EntryMsg::CancelForm)));

    scrollable(
        container(
            column![
                heading,
                templates_section,
                project_label,
                project_input,
                suggestion_list,
                hours_label,
                hours_input,
                notes_label,
                notes_input,
                error,
                row![cancel_btn, Space::new().width(Length::Fill), submit_btn],
            ]
            .spacing(6),
        )
        .padding(12)
        .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// ── Date picker calendar ──────────────────────────────────────────────────────

fn date_picker_view(state: &EasyHarvest) -> Element<'_, Message> {
    let today = Local::now().naive_local().date();
    let m = state.date_picker.month;
    let year = m.year();
    let month = m.month();

    let first_day = NaiveDate::from_ymd_opt(year, month, 1).expect("valid first day");
    let offset = first_day.weekday().num_days_from_monday() as usize;
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).expect("valid next year")
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).expect("valid next month")
    };
    let days_in_month = (next_month - first_day).num_days() as usize;

    // Month header
    let header = row![
        nav_arrow_btn("‹").on_press(Message::Nav(NavMsg::DatePickerMonthPrev)),
        Space::new().width(Length::Fill),
        text(format!("{} {year}", month_name(month)))
            .font(FONT_SEMIBOLD)
            .size(15)
            .color(TEXT_PRIMARY),
        Space::new().width(Length::Fill),
        nav_arrow_btn("›").on_press(Message::Nav(NavMsg::DatePickerMonthNext)),
    ]
    .align_y(Alignment::Center);

    // Weekday column headers
    let dow_cells: Vec<Element<Message>> = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
        .iter()
        .map(|d| {
            container(text(*d).font(FONT_MEDIUM).size(12).color(TEXT_MUTED))
                .width(Length::FillPortion(1))
                .align_x(Alignment::Center)
                .into()
        })
        .collect();

    // Week rows
    let total_cells = offset + days_in_month;
    let num_weeks = total_cells.div_ceil(7);

    let week_rows: Vec<Element<Message>> = (0..num_weeks)
        .map(|week| {
            let cells: Vec<Element<Message>> = (0..7usize)
                .map(|dow| {
                    let idx = week * 7 + dow;
                    if idx < offset || idx >= offset + days_in_month {
                        container(Space::new())
                            .width(Length::FillPortion(1))
                            .into()
                    } else {
                        let day = (idx - offset + 1) as u32;
                        let date = NaiveDate::from_ymd_opt(year, month, day).expect("valid day of month");
                        let is_selected = date == state.current_date;
                        let is_today = date == today;
                        let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);

                        let txt_color = if is_selected {
                            Color::WHITE
                        } else if is_today {
                            ACCENT
                        } else if is_weekend {
                            TEXT_MUTED
                        } else {
                            TEXT_PRIMARY
                        };
                        let bg = if is_selected { Some(ACCENT) } else { None };
                        let ring = is_today && !is_selected;

                        button(
                            container(
                                text(day.to_string()).font(FONT_MEDIUM).size(13).color(txt_color),
                            )
                            .width(Length::Fill)
                            .align_x(Alignment::Center),
                        )
                        .style(move |_, status: button::Status| button::Style {
                            background: Some(iced::Background::Color(
                                if let Some(c) = bg {
                                    c
                                } else if matches!(status, button::Status::Hovered) {
                                    SURFACE_RAISED
                                } else {
                                    Color::TRANSPARENT
                                },
                            )),
                            text_color: txt_color,
                            border: iced::Border {
                                color: if ring { ACCENT } else { Color::TRANSPARENT },
                                width: if ring { 1.0 } else { 0.0 },
                                radius: 6.0.into(),
                            },
                            ..Default::default()
                        })
                        .padding([4, 2])
                        .width(Length::FillPortion(1))
                        .on_press(Message::Nav(NavMsg::DatePickerSelect(date)))
                        .into()
                    }
                })
                .collect();
            row(cells).spacing(2).into()
        })
        .collect();

    let mut grid = column![header, Space::new(), row(dow_cells), Space::new()]
        .spacing(3);
    for wr in week_rows {
        grid = grid.push(wr);
    }

    container(grid)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(SURFACE)),
            border: iced::Border { radius: 0.0.into(), ..Default::default() },
            shadow: iced::Shadow {
                color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 10.0,
            },
            ..Default::default()
        })
        .padding([10, 16])
        .width(Length::Fill)
        .into()
}

fn status_badge(label: &str, color: Color) -> Element<'static, Message> {
    container(
        text(label.to_string())
            .font(FONT_MEDIUM)
            .size(11)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(with_alpha(color, 0.12))),
        border: iced::Border { color, width: 1.0, radius: 3.0.into() },
        ..Default::default()
    })
    .padding([1, 5])
    .into()
}

// end of day_view
