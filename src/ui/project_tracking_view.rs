use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input, Space,
};
use iced::{Alignment, Color, Element, Length};
use crate::state::project_budgets::BudgetScope;
use crate::state::project_tracking_layout::CardKey;

use crate::app::{
    managed_card_title, visible_and_hidden_cards, BudgetSummary, DragState, EasyHarvest,
    ManagedBudget, Message, PersonSplit, ProjectTrackingMsg, SplitState, ACCENT, BACKGROUND, DANGER,
    FONT_MEDIUM, FONT_REGULAR, FONT_SEMIBOLD, SUCCESS, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{
    badge, caption, card_style, delete_chip_btn, dropdown_container_style, field_label,
    format_hhmm, input_style, nav_arrow_btn, outline_btn_sm, primary_btn,
    progress_bar, refresh_btn, section_heading, stacked_progress_bar, suggestion_btn_style,
    toggle_active_style, toggle_inactive_style, LIST_ROW_SPACING, OTHERS_COLOR, PAGE_PADDING,
    SECTION_GAP, SPLIT_PALETTE,
};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let pt = &state.project_tracking;
    let year = pt.year;
    let any_loading = state.loading || pt.managed_loading;

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
            if any_loading {
                None
            } else {
                Some(Message::ProjectTracking(ProjectTrackingMsg::Refresh))
            },
        ),
    ]
    .align_y(Alignment::Center);

    let add_btn_row: Element<Message> = row![
        Space::new().width(Length::Fill),
        if pt.form.is_some() {
            Element::from(
                outline_btn_sm("Cancel")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::HideForm)),
            )
        } else {
            Element::from(
                outline_btn_sm("+ Add manual budget")
                    .on_press(Message::ProjectTracking(ProjectTrackingMsg::ShowForm)),
            )
        },
    ]
    .into();

    let mut content = column![year_row, add_btn_row].spacing(SECTION_GAP);

    // Add/edit form
    if let Some(form) = &pt.form {
        content = content.push(budget_form(state, form));
    }

    let (visible, hidden) = visible_and_hidden_cards(pt);

    if visible.is_empty() && hidden.is_empty() {
        let message = if any_loading {
            "Loading…"
        } else {
            "No managed projects with an hour budget and no manual budgets for this year."
        };
        content = content.push(text(message).font(FONT_REGULAR).size(13).color(TEXT_MUTED));
    } else if !visible.is_empty() {
        content = content.push(card_list(state, &visible));
    }

    if !hidden.is_empty() {
        content = content.push(
            outline_btn_sm_owned(format!(
                "{} Hidden ({})",
                if pt.show_hidden { "▾" } else { "▸" },
                hidden.len()
            ))
            .on_press(Message::ProjectTracking(ProjectTrackingMsg::ToggleShowHidden)),
        );
        // Hidden cards collapse at the end; expanded they show in full (so old
        // budgets can still be checked), with Unhide instead of Hide.
        if pt.show_hidden {
            let cards: Vec<Element<Message>> =
                hidden.iter().filter_map(|&key| card(state, key, true)).collect();
            content = content.push(column(cards).spacing(LIST_ROW_SPACING));
        }
    }

    // Extra right padding keeps cards clear of the vertical scrollbar; bottom
    // padding stops the last card sitting flush against the window edge.
    scrollable(content.padding(iced::Padding { right: 20.0, bottom: 16.0, ..PAGE_PADDING }))
        .height(Length::Fill)
        .into()
}

/// `outline_btn_sm` with an owned label (the label borrows otherwise).
fn outline_btn_sm_owned<'a>(label: String) -> button::Button<'a, Message> {
    button(text(label).font(FONT_MEDIUM).size(12).color(TEXT_MUTED))
        .style(super::outline_btn_style)
        .padding([8, 12])
}

/// Visible cards with drag-and-drop reordering. Each card is preceded by an
/// insertion slot (always present, so the layout doesn't shift mid-drag); a
/// taller trailing slot is added while dragging to drop after the last card.
fn card_list<'a>(state: &'a EasyHarvest, visible: &[CardKey]) -> Element<'a, Message> {
    let drag = state.project_tracking.drag;
    let msg = |m| Message::ProjectTracking(m);
    let mut list = column![].width(Length::Fill);
    for (i, &key) in visible.iter().enumerate() {
        let Some(card) = card(state, key, false) else { continue };
        let card: Element<Message> = if drag.is_some_and(|d| d.from == key) {
            stack![card, dimmed_overlay()].into()
        } else {
            card
        };
        let mut target = mouse_area(card);
        if drag.is_some() {
            target = target.on_enter(msg(ProjectTrackingMsg::DragOver(i)));
        }
        list = list
            .push(insertion_slot(drag, i, LIST_ROW_SPACING, Alignment::Center))
            .push(target);
    }
    // The list is always wrapped (only the handlers change) so starting a
    // drag doesn't rebuild the widget tree and reset hover states.
    if drag.is_none() {
        return mouse_area(list).into();
    }
    let len = visible.len();
    list = list.push(
        mouse_area(insertion_slot(drag, len, 24.0, Alignment::Start))
            .on_enter(msg(ProjectTrackingMsg::DragOver(len))),
    );
    // Leaving the list clears the slot, so a release outside cancels.
    mouse_area(list)
        .on_exit(msg(ProjectTrackingMsg::DragLeave))
        .into()
}

/// Gap of `height` in which a 2 px accent line (placed per `align`) marks the
/// drop position when `slot` is the drag's current insertion slot.
fn insertion_slot(
    drag: Option<DragState>,
    slot: usize,
    height: f32,
    align: Alignment,
) -> Element<'static, Message> {
    let active = drag.is_some_and(|d| d.over == Some(slot));
    let line = container(Space::new().width(Length::Fill).height(2))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: active.then_some(iced::Background::Color(ACCENT)),
            ..Default::default()
        });
    container(line)
        .width(Length::Fill)
        .height(height)
        .align_y(align)
        .into()
}

/// Translucent layer over the dragged card, so it reads as lifted out.
fn dimmed_overlay() -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(BACKGROUND.scale_alpha(0.6))),
            border: iced::Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

fn card(state: &EasyHarvest, key: CardKey, hidden: bool) -> Option<Element<'_, Message>> {
    let pt = &state.project_tracking;
    match key {
        CardKey::Harvest(pid) => {
            let m = pt.managed.iter().find(|m| m.project_id == pid)?;
            Some(managed_card(m, pt.managed_splits.get(&pid), hidden))
        }
        CardKey::Manual(id) => {
            if let Some(summary) = pt.summaries.iter().find(|s| s.budget.id == id) {
                return Some(budget_card(summary, hidden));
            }
            let b = pt.budgets.budgets_for(pt.year).iter().find(|b| b.id == id)?;
            Some(empty_budget_card(b.id, &b.name, b.budget_hours, state.loading, hidden))
        }
    }
}

/// Shared card container with the drag handle at the left. Only the handle
/// starts a drag, so the card's buttons keep working normally.
fn card_shell(
    body: iced::widget::Column<'_, Message>,
    key: CardKey,
    hidden: bool,
) -> Element<'_, Message> {
    // Hidden cards can't be reordered, so they get no drag handle.
    let content: Element<Message> = if hidden {
        body.width(Length::Fill).into()
    } else {
        let handle = mouse_area(
            container(text("⠿").size(16).color(TEXT_MUTED)).padding([2, 2]),
        )
        .on_press(Message::ProjectTracking(ProjectTrackingMsg::DragStart(key)))
        .interaction(iced::mouse::Interaction::Grab);
        row![handle, body.width(Length::Fill)]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
    };
    container(content)
    .style(card_style)
    .padding([12, 14])
    .width(Length::Fill)
    .into()
}

fn usage_color(pct: f64) -> Color {
    if pct > 1.0 {
        DANGER
    } else if pct >= 0.8 {
        ACCENT
    } else {
        SUCCESS
    }
}

fn remaining_label(remaining_hours: f64) -> Element<'static, Message> {
    let (label, color) = if remaining_hours >= 0.0 {
        (format!("{} remaining", format_hhmm(remaining_hours)), SUCCESS)
    } else {
        (format!("{} over budget", format_hhmm(-remaining_hours)), DANGER)
    };
    text(label).font(FONT_SEMIBOLD).size(13).color(color).into()
}

/// "Hide" on visible cards, "Unhide" on cards in the hidden list.
fn visibility_btn<'a>(key: CardKey, hidden: bool) -> button::Button<'a, Message> {
    if hidden {
        outline_btn_sm("Unhide").on_press(Message::ProjectTracking(ProjectTrackingMsg::UnhideCard(key)))
    } else {
        outline_btn_sm("Hide").on_press(Message::ProjectTracking(ProjectTrackingMsg::HideCard(key)))
    }
}

/// Segments with positive hours paired with their colour: named segments take
/// `SPLIT_PALETTE` in order, "Others" (no user id) takes `OTHERS_COLOR`.
fn colored_segments(split: &PersonSplit) -> Vec<(&crate::app::Segment, Color)> {
    let mut named = 0usize;
    split
        .segments
        .iter()
        .filter(|s| s.hours > 0.0)
        .map(|s| {
            let color = if s.user_id.is_some() {
                let c = SPLIT_PALETTE[named % SPLIT_PALETTE.len()];
                named += 1;
                c
            } else {
                OTHERS_COLOR
            };
            (s, color)
        })
        .collect()
}

/// Stacked bar fractions: hours / budget, or hours / used when over budget so
/// the bar fills 100 %.
fn split_fractions(segments: &[(&crate::app::Segment, Color)], budget: f64, used: f64) -> Vec<(f32, Color)> {
    let denom = budget.max(used);
    if denom <= 0.0 {
        return Vec::new();
    }
    segments.iter().map(|(s, c)| ((s.hours / denom) as f32, *c)).collect()
}

fn split_legend<'a>(segments: &[(&'a crate::app::Segment, Color)], approximate: bool) -> Element<'a, Message> {
    let prefix = if approximate { "≈ " } else { "" };
    let items: Vec<Element<Message>> = segments
        .iter()
        .map(|(s, color)| {
            row![
                text("■").font(FONT_REGULAR).size(12).color(*color),
                text(format!("{} {prefix}{}", s.label, format_hhmm(s.hours)))
                    .font(FONT_REGULAR)
                    .size(12)
                    .color(TEXT_MUTED),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .into()
        })
        .collect();
    iced::widget::Row::with_children(items).spacing(12).wrap().into()
}

fn managed_card<'a>(
    m: &'a ManagedBudget,
    split: Option<&'a SplitState>,
    hidden: bool,
) -> Element<'a, Message> {
    let key = CardKey::Harvest(m.project_id);
    let pct = if m.budget_hours > 0.0 { m.spent_hours / m.budget_hours } else { 0.0 };
    let bar_color = usage_color(pct);

    // The title takes the remaining width and wraps, so long project names
    // never push the Hide button out of the card. Spent / budget lives in the
    // footer row for the same reason.
    let name = text(managed_card_title(m)).font(FONT_SEMIBOLD).size(14).color(TEXT_PRIMARY);
    let name: Element<Message> = if m.personal {
        row![name, badge("Personal")].spacing(6).align_y(Alignment::Center).wrap().into()
    } else {
        name.into()
    };
    let mut title = column![name].spacing(2).width(Length::Fill);
    if m.is_monthly {
        title = title.push(caption("this month"));
    }
    let header = row![title, visibility_btn(key, hidden)]
        .spacing(6)
        .align_y(Alignment::Center);

    let plain_bar = || progress_bar(pct.min(1.0) as f32, bar_color, 6);
    let mut body = column![header].spacing(8);
    match split {
        // A personal budget only ever holds the user's own hours: plain bar.
        Some(SplitState::Ready(_)) if m.personal => body = body.push(plain_bar()),
        Some(SplitState::Ready(split)) => {
            let segments = colored_segments(split);
            if segments.is_empty() {
                body = body.push(plain_bar());
            } else {
                body = body
                    .push(stacked_progress_bar(
                        &split_fractions(&segments, m.budget_hours, m.spent_hours),
                        6,
                    ))
                    .push(split_legend(&segments, split.approximate));
            }
        }
        Some(SplitState::Failed) => body = body.push(plain_bar()),
        Some(SplitState::Loading) | None => {
            let loading: Element<Message> = caption("loading split…").into();
            body = body.push(plain_bar()).push(loading);
        }
    }

    body = body.push(
        row![
            remaining_label(m.remaining_hours),
            Space::new().width(Length::Fill),
            text(format!("{} / {}", format_hhmm(m.spent_hours), format_hhmm(m.budget_hours)))
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
            text(format!("{:.0}%", pct * 100.0))
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(bar_color),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );

    card_shell(body, key, hidden)
}

fn manual_header(id: u64, name: &str, hidden: bool) -> iced::widget::Row<'_, Message> {
    row![
        // Fill width so a long name wraps instead of pushing the buttons out.
        container(
            row![
                text(name.to_owned())
                    .font(FONT_SEMIBOLD)
                    .size(14)
                    .color(TEXT_PRIMARY),
                badge("Manual"),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .wrap(),
        )
        .width(Length::Fill),
        outline_btn_sm("Edit")
            .on_press(Message::ProjectTracking(ProjectTrackingMsg::EditBudget(id))),
        visibility_btn(CardKey::Manual(id), hidden),
        outline_btn_sm("Delete")
            .on_press(Message::ProjectTracking(ProjectTrackingMsg::DeleteBudget(id))),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
}

/// Split segments a manual card shows as a stacked bar with legend. Empty
/// (plain usage-coloured bar) when the effective scope is Me or at most one
/// person has hours.
fn manual_split_segments(summary: &BudgetSummary) -> Vec<(&crate::app::Segment, Color)> {
    if summary.budget.scope == BudgetScope::Me {
        return Vec::new();
    }
    let segments = colored_segments(&summary.split);
    if segments.len() <= 1 { Vec::new() } else { segments }
}

fn budget_card(summary: &BudgetSummary, hidden: bool) -> Element<'_, Message> {
    let pct = summary.pct_used;
    let bar_color = usage_color(pct);

    let segments = manual_split_segments(summary);
    let bar = if segments.is_empty() {
        progress_bar(pct.min(1.0) as f32, bar_color, 6)
    } else {
        stacked_progress_bar(
            &split_fractions(&segments, summary.budget.budget_hours, summary.used_hours),
            6,
        )
    };

    let mut body = column![manual_header(summary.budget.id, &summary.budget.name, hidden), bar].spacing(8);
    if !segments.is_empty() {
        body = body.push(split_legend(&segments, summary.split.approximate));
    }
    body = body.push(
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
            remaining_label(summary.remaining_hours),
            Space::new().width(Length::Fill),
            text(format!("{:.0}%", pct * 100.0))
                .font(FONT_SEMIBOLD)
                .size(13)
                .color(bar_color),
        ]
        .align_y(Alignment::Center),
    );

    card_shell(body, CardKey::Manual(summary.budget.id), hidden)
}

fn empty_budget_card(
    id: u64,
    name: &str,
    budget_hours: f64,
    loading: bool,
    hidden: bool,
) -> Element<'_, Message> {
    let status = if loading { "loading…" } else { "press Refresh to load data" };
    card_shell(
        column![
            manual_header(id, name, hidden),
            text(format!("Budget: {} — {status}", format_hhmm(budget_hours)))
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
        ]
        .spacing(6),
        CardKey::Manual(id),
        hidden,
    )
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

    let scope_toggle: Element<Message> = if state.settings.team_lead_mode
        && !state.team_settings.members.is_empty()
    {
        let btn = |label: &'static str, scope: BudgetScope| {
            let active = form.scope == scope;
            button(text(label).font(FONT_MEDIUM).size(12).align_x(Alignment::Center))
                .style(move |_, _: iced::widget::button::Status| {
                    if active { toggle_active_style(5.0) } else { toggle_inactive_style(5.0) }
                })
                .padding([4, 6])
                .width(Length::FillPortion(1))
                .on_press(Message::ProjectTracking(ProjectTrackingMsg::ScopeChanged(scope)))
        };
        column![
            field_label("Count hours of"),
            row![btn("Me", BudgetScope::Me), btn("Me + team", BudgetScope::MeAndTeam)].spacing(4),
        ]
        .spacing(8)
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
            scope_toggle,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Segment;
    use crate::state::project_budgets::ProjectBudget;

    fn summary(scope: BudgetScope, people: &[(i64, f64)]) -> BudgetSummary {
        let segments = people
            .iter()
            .map(|&(id, hours)| Segment { user_id: Some(id), label: format!("U{id}"), hours })
            .collect();
        BudgetSummary {
            budget: ProjectBudget {
                id: 1,
                name: "Edu".into(),
                budget_hours: 10.0,
                project_ids: vec![1],
                task_ids: vec![],
                scope,
            },
            used_hours: people.iter().map(|p| p.1).sum(),
            remaining_hours: 0.0,
            pct_used: 1.2,
            split: PersonSplit { segments, approximate: false },
        }
    }

    #[test]
    fn me_scope_manual_card_uses_plain_bar() {
        assert!(manual_split_segments(&summary(BudgetScope::Me, &[(1, 12.0)])).is_empty());
        assert!(manual_split_segments(&summary(BudgetScope::Me, &[(1, 6.0), (2, 6.0)])).is_empty());
    }

    #[test]
    fn team_scope_manual_card_with_one_person_uses_plain_bar() {
        assert!(manual_split_segments(&summary(BudgetScope::MeAndTeam, &[(1, 12.0)])).is_empty());
        assert!(
            manual_split_segments(&summary(BudgetScope::MeAndTeam, &[(1, 12.0), (2, 0.0)])).is_empty()
        );
    }

    #[test]
    fn team_scope_manual_card_with_several_people_is_stacked() {
        let s = summary(BudgetScope::MeAndTeam, &[(1, 6.0), (2, 6.0)]);
        assert_eq!(manual_split_segments(&s).len(), 2);
    }
}
