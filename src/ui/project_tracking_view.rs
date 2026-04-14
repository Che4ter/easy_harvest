use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};

use crate::app::{
    EasyHarvest, Message, ProjectTrackingMsg, ACCENT, DANGER, FONT_REGULAR,
    FONT_SEMIBOLD, SUCCESS, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{
    caption, card_style, delete_chip_btn, dropdown_container_style, field_label, format_hhmm,
    input_style, nav_arrow_btn, outline_btn_sm, primary_btn, progress_bar, refresh_btn,
    section_heading, suggestion_btn_style, LIST_ROW_SPACING, PAGE_PADDING, SECTION_GAP,
};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let year = state.project_tracking.year;

    let year_row = row![
        nav_arrow_btn("‹").on_press(Message::ProjectTracking(ProjectTrackingMsg::YearPrev)),
        Space::new().width(10).height(10),
        text(year.to_string())
            .font(FONT_SEMIBOLD)
            .size(18)
            .color(TEXT_PRIMARY),
        Space::new().width(10).height(10),
        nav_arrow_btn("›").on_press(Message::ProjectTracking(ProjectTrackingMsg::YearNext)),
        Space::new().width(Length::Fill),
        refresh_btn("↻  Refresh").on_press_maybe(
            if state.loading {
                None
            } else {
                Some(Message::ProjectTracking(ProjectTrackingMsg::Refresh))
            },
        ),
    ]
    .align_y(Alignment::Center);

    let add_btn_row: Element<Message> = row![
        Space::new().width(Length::Fill),
        if state.project_tracking.form.is_some() {
            Element::from(
                outline_btn_sm("Cancel")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::HideForm)),
            )
        } else {
            Element::from(
                outline_btn_sm("+ Add Budget")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::ShowForm)),
            )
        },
    ]
    .into();

    if state.loading {
        return scrollable(
            column![
                year_row,
                add_btn_row,
                text("Loading…")
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

    let mut content = column![year_row, add_btn_row].spacing(SECTION_GAP);

    // Add/edit form
    if let Some(form) = &state.project_tracking.form {
        content = content.push(budget_form(state, form));
    }

    let budgets = state.project_tracking.budgets.budgets_for(year);

    // Budget cards
    if state.project_tracking.summaries.is_empty() && budgets.is_empty() {
        content = content.push(
            text("No project budgets defined for this year. Add one to start tracking hours.")
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
        );
    } else if state.project_tracking.summaries.is_empty() {
        let cards: Vec<Element<Message>> = budgets
            .iter()
            .map(|b| empty_budget_card(b.id, &b.name, b.budget_hours))
            .collect();
        content = content.push(column(cards).spacing(LIST_ROW_SPACING));
    } else {
        let cards: Vec<Element<Message>> = state
            .project_tracking
            .summaries
            .iter()
            .map(|s| budget_card(s))
            .collect();
        content = content.push(column(cards).spacing(LIST_ROW_SPACING));
    }

    scrollable(content.padding(PAGE_PADDING))
        .height(Length::Fill)
        .into()
}

fn budget_card(summary: &crate::app::BudgetSummary) -> Element<'_, Message> {
    let pct = summary.pct_used;
    let bar_color = if pct > 1.0 {
        DANGER
    } else if pct >= 0.8 {
        ACCENT
    } else {
        SUCCESS
    };

    let remaining_label = if summary.remaining_hours >= 0.0 {
        format!("{} remaining", format_hhmm(summary.remaining_hours))
    } else {
        format!("{} over budget", format_hhmm(-summary.remaining_hours))
    };

    let remaining_color = if summary.remaining_hours >= 0.0 {
        SUCCESS
    } else {
        DANGER
    };

    let id = summary.budget.id;
    container(
        column![
            row![
                text(summary.budget.name.clone())
                    .font(FONT_SEMIBOLD)
                    .size(14)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
                outline_btn_sm("Edit")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::EditBudget(id))),
                outline_btn_sm("Delete")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::DeleteBudget(id))),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            progress_bar(pct.min(1.0) as f32, bar_color, 6),
            row![
                text(format!(
                    "{} of {} used",
                    format_hhmm(summary.used_hours),
                    format_hhmm(summary.budget.budget_hours),
                ))
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
                Space::new().width(8),
                text("·").font(FONT_REGULAR).size(13).color(TEXT_MUTED),
                Space::new().width(8),
                text(remaining_label)
                    .font(FONT_SEMIBOLD)
                    .size(13)
                    .color(remaining_color),
                Space::new().width(Length::Fill),
                text(format!("{:.0}%", pct * 100.0))
                    .font(FONT_SEMIBOLD)
                    .size(13)
                    .color(bar_color),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(8),
    )
    .style(card_style)
    .padding([12, 14])
    .width(Length::Fill)
    .into()
}

fn empty_budget_card(id: u64, name: &str, budget_hours: f64) -> Element<'_, Message> {
    container(
        column![
            row![
                text(name.to_owned())
                    .font(FONT_SEMIBOLD)
                    .size(14)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
                outline_btn_sm("Edit")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::EditBudget(id))),
                outline_btn_sm("Delete")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::DeleteBudget(id))),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            text(format!("Budget: {} — press Refresh to load data", format_hhmm(budget_hours)))
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
        ]
        .spacing(6),
    )
    .style(card_style)
    .padding([12, 14])
    .width(Length::Fill)
    .into()
}

fn budget_form<'a>(state: &'a EasyHarvest, form: &'a crate::app::BudgetForm) -> Element<'a, Message> {
    let title = if form.editing_id.is_some() {
        "Edit Budget"
    } else {
        "New Budget"
    };

    // Project search suggestions
    let query = form.project_query.to_lowercase();
    let selected_ids: Vec<i64> = form.selected_projects.iter().map(|(id, _, _)| *id).collect();
    let suggestions: Vec<(usize, &str, &str)> = state
        .assignments
        .iter()
        .filter(|a| a.is_active)
        .enumerate()
        .filter(|(_, a)| !selected_ids.contains(&a.project.id))
        .filter(|(_, a)| {
            !query.is_empty()
                && (a.project.name.to_lowercase().contains(&query)
                    || a.client.name.to_lowercase().contains(&query))
        })
        .take(6)
        .map(|(i, a)| (i, a.project.name.as_str(), a.client.name.as_str()))
        .collect();

    let suggestion_list: Element<Message> = if !suggestions.is_empty() {
        let items: Vec<Element<Message>> = suggestions
            .iter()
            .map(|&(idx, proj, client)| {
                button(
                    text(format!("{proj} — {client}"))
                        .font(FONT_REGULAR)
                        .size(13)
                        .color(TEXT_PRIMARY),
                )
                .style(suggestion_btn_style)
                .padding([8, 12])
                .width(Length::Fill)
                .on_press(Message::ProjectTracking(ProjectTrackingMsg::ProjectSelected(idx)))
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

    // Selected project chips
    let chips: Vec<Element<Message>> = form
        .selected_projects
        .iter()
        .map(|(pid, name, client)| {
            let pid = *pid;
            row![
                container(
                    text(format!("{name} — {client}"))
                        .font(FONT_REGULAR)
                        .size(12)
                        .color(TEXT_PRIMARY),
                )
                .style(|_| super::raised_container_style(4.0))
                .padding([4, 8]),
                delete_chip_btn(Message::ProjectTracking(ProjectTrackingMsg::ProjectRemoved(pid))),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    let chips_row: Element<Message> = if chips.is_empty() {
        caption("No projects selected").into()
    } else {
        column(chips).spacing(4).into()
    };

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
            section_heading(title),
            field_label("Name"),
            text_input("Budget name", &form.name_input)
                .on_input(|v| Message::ProjectTracking(ProjectTrackingMsg::NameChanged(v)))
                .style(input_style)
                .size(13)
                .padding([8, 10]),
            field_label("Budget Hours"),
            text_input("e.g. 200", &form.budget_hours_input)
                .on_input(|v| Message::ProjectTracking(ProjectTrackingMsg::BudgetHoursChanged(v)))
                .style(input_style)
                .size(13)
                .padding([8, 10]),
            field_label("Projects"),
            text_input("Search projects…", &form.project_query)
                .on_input(|v| Message::ProjectTracking(ProjectTrackingMsg::ProjectQueryChanged(v)))
                .style(input_style)
                .size(13)
                .padding([8, 10]),
            suggestion_list,
            chips_row,
            error_el,
            primary_btn("Save Budget")
                .on_press(Message::ProjectTracking(ProjectTrackingMsg::FormSubmit)),
        ]
        .spacing(8),
    )
    .style(card_style)
    .padding(14)
    .width(Length::Fill)
    .into()
}
