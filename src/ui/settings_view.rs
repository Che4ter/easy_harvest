use chrono::{Datelike, Local};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length};

use crate::app::{
    EasyHarvest, EntryMsg, Message, SettingsMsg, ACCENT, DANGER, FONT_MEDIUM, FONT_REGULAR, FONT_SEMIBOLD,
    SUCCESS, SURFACE, TEXT_MUTED, TEXT_PRIMARY,
};
use super::{
    caption, card_style, delete_chip_btn, dropdown_container_style, field_label,
    input_style, month_abbr, nav_arrow_btn, outline_btn, outline_btn_sm,
    outline_btn_style, primary_btn, section_heading, suggestion_btn_style,
    toggle_active_style, toggle_inactive_style, with_alpha,
    PAGE_PADDING, SECTION_GAP, LIST_ROW_SPACING,
};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    if state.client.is_none() {
        return if state.wizard_step == 0 {
            wizard_data_folder(state)
        } else {
            wizard_credentials(state)
        };
    }

    scrollable(
        column![
            sync_section(state),
            profile_section(state),
            carryover_section(state),
            holidays_section(state),
            holiday_tasks_section(state),
            templates_section(state),
            data_dir_section(state),
            startup_section(state),
            connection_section(state),
            container(
                caption(concat!("Easy Harvest v", env!("CARGO_PKG_VERSION"))),
            )
            .width(Length::Fill)
            .center_x(Length::Fill),
        ]
        .spacing(SECTION_GAP)
        .padding(PAGE_PADDING),
    )
    .height(Length::Fill)
    .into()
}

// ── Wizard: step 0 — data folder ─────────────────────────────────────────────

fn wizard_data_folder(state: &EasyHarvest) -> Element<'_, Message> {
    let use_default_btn = outline_btn("Use Default")
        .on_press(Message::Settings(SettingsMsg::WizardUseDefault));

    let continue_btn = primary_btn("Continue  →")
        .on_press(Message::Settings(SettingsMsg::WizardNext));

    let card = container(
        column![
            text("Welcome to Easy Harvest")
                .font(FONT_SEMIBOLD)
                .size(20)
                .color(TEXT_PRIMARY),
            text(
                "Choose where to store your settings and time-tracking data. \
                 Pointing to a OneDrive or Dropbox folder lets you sync \
                 across all your devices.",
            )
            .font(FONT_REGULAR)
            .size(12)
            .color(TEXT_MUTED),
            field_label("Data Folder"),
            row![
                text_input("Path…", &state.settings_form.data_dir_input)
                    .on_input(|v| Message::Settings(SettingsMsg::DataDirChanged(v)))
                    .size(13)
                    .padding([8, 10])
                    .style(input_style),
                Space::new().width(8).height(8),
                outline_btn_sm("Browse…")
                    .on_press(Message::Settings(SettingsMsg::PickDataDir)),
            ]
            .align_y(Alignment::Center),
            row![
                use_default_btn,
                Space::new().width(Length::Fill),
                continue_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(18)
    .max_width(420);

    container(card).center(Length::Fill).into()
}

// ── Wizard: step 1 — Harvest credentials ─────────────────────────────────────

fn wizard_credentials(state: &EasyHarvest) -> Element<'_, Message> {
    let back_btn = outline_btn("←  Back")
        .on_press(Message::Settings(SettingsMsg::WizardBack));

    let card = container(
        column![
            text("Connect to Harvest")
                .font(FONT_SEMIBOLD)
                .size(20)
                .color(TEXT_PRIMARY),
            text(
                "Get your Personal Access Token and Account ID from \
                 harvestapp.com → Settings → Developers.",
            )
            .font(FONT_REGULAR)
            .size(12)
            .color(TEXT_MUTED),
            field_label("Personal Access Token"),
            token_input(state),
            field_label("Account ID"),
            account_input(state),
            row![
                back_btn,
                Space::new().width(Length::Fill),
                connect_btn(state),
            ]
            .align_y(Alignment::Center),
            connection_error(state),
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(18)
    .max_width(420);

    container(card).center(Length::Fill).into()
}

// ── Work Profile section ──────────────────────────────────────────────────────

fn profile_section(state: &EasyHarvest) -> Element<'_, Message> {
    let year = Local::now().year();
    let hpd = state.settings.expected_hours_per_day();
    let hpw = state.settings.expected_hours_per_week();
    let eff_hols = state.settings.effective_holiday_days_for(year);

    let summary = caption(format!(
        "{}/day · {}/week · {:.1} holiday days",
        super::format_hhmm(hpd), super::format_hhmm(hpw), eff_hols,
    ));

    let save_row: Element<Message> = {
        let btn = primary_btn("Save")
            .on_press(Message::Settings(SettingsMsg::SaveProfile));

        let feedback: Element<Message> = if state.settings_form.profile_saved {
            text("Saved!").font(FONT_MEDIUM).size(12).color(SUCCESS).into()
        } else if let Some(err) = &state.settings_form.profile_error {
            text(err.as_str()).font(FONT_REGULAR).size(12).color(DANGER).into()
        } else {
            Space::new().into()
        };

        row![btn, Space::new().width(10).height(10), feedback]
            .align_y(Alignment::Center)
            .into()
    };

    container(
        column![
            section_heading("Work Profile"),
            numeric_row(
                "Full-time weekly hours",
                &state.settings_form.weekly_hours_input,
                "41",
                |v| Message::Settings(SettingsMsg::WeeklyHoursChanged(v)),
                "h (100%)",
            ),
            numeric_row(
                "Work percentage",
                &state.settings_form.percentage_input,
                "100",
                |v| Message::Settings(SettingsMsg::PercentageChanged(v)),
                "%",
            ),
            numeric_row(
                "Vacation days / year",
                &state.settings_form.holidays_input,
                "25",
                |v| Message::Settings(SettingsMsg::HolidaysChanged(v)),
                "days (100%)",
            ),
            first_work_day_row(state),
            summary,
            save_row,
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

// ── Carryover section ─────────────────────────────────────────────────────────

fn carryover_section(state: &EasyHarvest) -> Element<'_, Message> {
    // Collect existing entries sorted by year descending
    let mut entries: Vec<(i32, &crate::state::settings::YearCarryover)> =
        state.settings.carryover.iter().map(|(y, c)| (*y, c)).collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0));

    let rows: Vec<Element<Message>> = entries
        .iter()
        .map(|(year, c)| {
            let year = *year;

            let del_btn = super::delete_chip_btn(Message::Settings(SettingsMsg::CarryoverDelete(year)));

            // Year chip
            let year_chip = container(
                text(year.to_string()).font(FONT_SEMIBOLD).size(13).color(ACCENT),
            )
            .style(|_| container::Style {
                background: Some(iced::Background::Color(Color {
                    r: ACCENT.r, g: ACCENT.g, b: ACCENT.b, a: 0.12,
                })),
                border: iced::Border {
                    color: with_alpha(ACCENT, 0.30),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .padding([4, 10]);

            let ot_color = if c.overtime_hours >= 0.0 { SUCCESS } else { DANGER };

            let values = row![
                caption("Vacation"),
                Space::new().width(4).height(4),
                text(format!("{:.1}d", c.holiday_days))
                    .font(FONT_SEMIBOLD).size(12).color(TEXT_PRIMARY),
                Space::new().width(16).height(16),
                container(Space::new().width(1).height(1))
                    .style(|_| container::Style {
                        background: Some(iced::Background::Color(TEXT_MUTED)),
                        ..Default::default()
                    })
                    .width(1)
                    .height(12),
                Space::new().width(16).height(16),
                caption("Overtime"),
                Space::new().width(4).height(4),
                text(format!("{:+.1}h", c.overtime_hours))
                    .font(FONT_SEMIBOLD).size(12).color(ot_color),
            ]
            .align_y(Alignment::Center);

            container(
                row![year_chip, Space::new().width(12).height(12), values, Space::new().width(Length::Fill), del_btn]
                    .align_y(Alignment::Center),
            )
            .style(|_| container::Style {
                background: Some(iced::Background::Color(SURFACE)),
                border: iced::Border { radius: 8.0.into(), ..Default::default() },
                ..Default::default()
            })
            .padding([8, 10])
            .width(Length::Fill)
            .into()
        })
        .collect();

    let list: Element<Message> = if rows.is_empty() {
        text("No carryover entries yet.")
            .font(FONT_REGULAR).size(12).color(TEXT_MUTED).into()
    } else {
        column(rows).spacing(LIST_ROW_SPACING).into()
    };

    // Add form — three labeled fields + button
    let feedback: Element<Message> = if let Some(err) = &state.settings_form.carryover_error {
        text(err.as_str()).font(FONT_REGULAR).size(12).color(DANGER).into()
    } else {
        Space::new().into()
    };

    let add_form = row![
        // Year field
        column![
            caption("Year"),
            text_input("e.g. 2026", &state.settings_form.carryover_year_input)
                .on_input(|v| Message::Settings(SettingsMsg::CarryoverYearChanged(v)))
                .size(13).padding([7, 8]).style(input_style).width(64),
        ]
        .spacing(4),
        Space::new().width(8).height(8),
        // Holiday days field
        column![
            caption("Vacation days"),
            text_input("0.0", &state.settings_form.carryover_holiday_input)
                .on_input(|v| Message::Settings(SettingsMsg::CarryoverHolidayChanged(v)))
                .size(13).padding([7, 8]).style(input_style).width(72),
        ]
        .spacing(4),
        Space::new().width(8).height(8),
        // OT hours field
        column![
            caption("Overtime hours"),
            text_input("0.0", &state.settings_form.carryover_overtime_input)
                .on_input(|v| Message::Settings(SettingsMsg::CarryoverOvertimeChanged(v)))
                .size(13).padding([7, 8]).style(input_style).width(72),
        ]
        .spacing(4),
        Space::new().width(12).height(12),
        // Add button aligned to bottom of fields
        column![
            Space::new(), // matches label + gap height
            primary_btn("+ Add")
                .on_press(Message::Settings(SettingsMsg::CarryoverSave)),
        ],
    ]
    .align_y(Alignment::End);

    container(
        column![
            section_heading("Carryover"),
            caption("Balances brought over from the previous year."),
            list,
            add_form,
            feedback,
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

// ── Public Holidays section (read-only) ──────────────────────────────────────

fn holidays_section(state: &EasyHarvest) -> Element<'_, Message> {
    let year = state.settings_form.holiday_view_year;
    let holidays = &state.settings_form.cached_holidays;
    let epd = state.settings.expected_hours_per_day();

    let year_nav = row![
        nav_arrow_btn("‹")
            .on_press(Message::Settings(SettingsMsg::HolidayViewYearPrev)),
        text(year.to_string()).font(FONT_SEMIBOLD).size(13).color(TEXT_PRIMARY),
        nav_arrow_btn("›")
            .on_press(Message::Settings(SettingsMsg::HolidayViewYearNext)),
    ]
    .align_y(Alignment::Center)
    .spacing(4);

    let rows: Vec<Element<Message>> = holidays
        .iter()
        .map(|h| {
            let date_str = format!(
                "{:2} {}",
                h.date.day(),
                month_abbr(h.date.month())
            );
            let credit = h.credit_hours(epd);
            let credit_str = if h.half_day {
                format!("½ day  ({:.1}h)", credit)
            } else {
                format!("1 day  ({:.1}h)", credit)
            };
            row![
                text(date_str)
                    .font(FONT_REGULAR)
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(88),
                text(h.name.clone())
                    .font(FONT_MEDIUM)
                    .size(12)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
                caption(credit_str),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    container(
        column![
            row![
                section_heading("Public Holidays"),
                Space::new().width(Length::Fill),
                year_nav,
            ]
            .align_y(Alignment::Center),
            caption("Swiss public holidays — used for working day calculations. Read-only."),
            column(rows).spacing(LIST_ROW_SPACING),
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}


// ── Holiday Tasks section ─────────────────────────────────────────────────────

// ── Sync assignments ──────────────────────────────────────────────────────────

fn sync_section(state: &EasyHarvest) -> Element<'_, Message> {
    let count_label = if state.assignments.is_empty() {
        text("Not loaded yet.").font(FONT_REGULAR).size(12).color(TEXT_MUTED)
    } else {
        let active = state.assignments.iter().filter(|a| a.is_active).count();
        text(format!("{active} active projects loaded."))
            .font(FONT_REGULAR).size(12).color(TEXT_MUTED)
    };

    let sync_btn = primary_btn("↻  Sync Assignments")
        .on_press(Message::Entry(Box::new(EntryMsg::SyncAssignments)));

    container(
        row![
            column![
                section_heading("Project Assignments"),
                count_label,
            ]
            .spacing(4)
            .width(Length::Fill),
            sync_btn,
        ]
        .align_y(Alignment::Center),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

fn holiday_tasks_section(state: &EasyHarvest) -> Element<'_, Message> {
    if state.assignments.is_empty() {
        return container(
            column![
                section_heading("Holiday Tasks"),
                text("No assignments loaded — use Sync Assignments above, or open the Day view first.")
                    .font(FONT_REGULAR)
                    .size(12)
                    .color(TEXT_MUTED),
            ]
            .spacing(SECTION_GAP),
        )
        .style(card_style)
        .padding(12)
        .width(Length::Fill)
        .into();
    }

    // Use cached deduped task list
    let tasks = &state.settings_form.cached_task_list;

    // Selected task chips with × to remove
    let chips: Vec<Element<Message>> = state
        .settings
        .holiday_task_ids
        .iter()
        .map(|&id| {
            let name = tasks.iter()
                .find(|(tid, ..)| *tid == id)
                .map(|(_, n, _)| n.clone())
                .unwrap_or_else(|| format!("#{id}"));
            container(
                row![
                    text(name).font(FONT_MEDIUM).size(12).color(TEXT_PRIMARY).width(Length::Fill),
                    delete_chip_btn(Message::Settings(SettingsMsg::HolidayTaskToggle(id))),
                ]
                .align_y(Alignment::Center),
            )
            .style(|_| container::Style {
                background: Some(iced::Background::Color(with_alpha(ACCENT, 0.10))),
                border: iced::Border {
                    color: with_alpha(ACCENT, 0.40),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .padding([5, 8])
            .width(Length::Fill)
            .into()
        })
        .collect();

    let chips_el: Element<Message> = if chips.is_empty() {
        text("No tasks selected — search below to add one.")
            .font(FONT_REGULAR).size(12).color(TEXT_MUTED).into()
    } else {
        column(chips).spacing(3).into()
    };

    // Search input
    let search_input = text_input("Search tasks to add…", &state.settings_form.holiday_task_query)
        .on_input(|v| Message::Settings(SettingsMsg::HolidayTaskQueryChanged(v)))
        .size(13)
        .padding([8, 10])
        .style(input_style);

    // Dropdown suggestions when query is non-empty
    let query = state.settings_form.holiday_task_query.to_lowercase();
    let suggestions_el: Element<Message> = if !query.is_empty() {
        let items: Vec<Element<Message>> = tasks
            .iter()
            .filter(|(id, name, _)| {
                !state.settings.holiday_task_ids.contains(id)
                    && name.to_lowercase().contains(&query)
            })
            .map(|(id, name, ctx)| {
                let task_id = *id;
                button(
                    column![
                        text(name.clone()).font(FONT_MEDIUM).size(13).color(TEXT_PRIMARY),
                        caption(ctx.clone()),
                    ]
                    .spacing(1),
                )
                .style(suggestion_btn_style)
                .padding([8, 12])
                .width(Length::Fill)
                .on_press(Message::Settings(SettingsMsg::HolidayTaskToggle(task_id)))
                .into()
            })
            .collect();

        if items.is_empty() {
            text("No matching tasks.")
                .font(FONT_REGULAR).size(12).color(TEXT_MUTED).into()
        } else {
            container(column(items).spacing(1))
                .style(dropdown_container_style)
                .padding(4)
                .width(Length::Fill)
                .into()
        }
    } else {
        Space::new().into()
    };

    container(
        column![
            section_heading("Holiday Tasks"),
            caption("Tasks that count as vacation time in the Vacation tab and overtime balance."),
            chips_el,
            search_input,
            suggestions_el,
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

// ── Data directory section ────────────────────────────────────────────────────

fn data_dir_section(state: &EasyHarvest) -> Element<'_, Message> {
    let browse_btn = outline_btn_sm("Browse…")
        .on_press(Message::Settings(SettingsMsg::PickDataDir));

    let save_row: Element<Message> = {
        let btn = primary_btn("Apply")
            .on_press(Message::Settings(SettingsMsg::SaveDataDir));

        let feedback: Element<Message> = if state.settings_form.data_dir_saved {
            text("Applied! Please restart the app for the change to take effect.")
                .font(FONT_MEDIUM).size(12).color(SUCCESS).into()
        } else {
            Space::new().into()
        };

        row![btn, Space::new().width(10).height(10), feedback]
            .align_y(Alignment::Center)
            .into()
    };

    container(
        column![
            section_heading("Data Folder"),
            caption("Your settings and local work-day records are stored here. Point this to a OneDrive or cloud folder to sync across devices."),
            row![
                text_input("Path…", &state.settings_form.data_dir_input)
                    .on_input(|v| Message::Settings(SettingsMsg::DataDirChanged(v)))
                    .size(13)
                    .padding([8, 10])
                    .style(input_style),
                Space::new().width(8).height(8),
                browse_btn,
            ]
            .align_y(Alignment::Center),
            save_row,
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

// ── Autostart section ─────────────────────────────────────────────────────────

fn startup_section(state: &EasyHarvest) -> Element<'_, Message> {
    if !cfg!(any(target_os = "linux", target_os = "windows")) {
        return Space::new().into();
    }

    let enabled = state.settings.autostart;
    let btn_label = if enabled { "Enabled" } else { "Disabled" };

    let toggle = button(text(btn_label).font(FONT_MEDIUM).size(12))
        .style(move |_, _: button::Status| {
            if enabled { toggle_active_style(6.0) } else { toggle_inactive_style(6.0) }
        })
        .padding([5, 14])
        .on_press(Message::Settings(SettingsMsg::AutostartToggle));

    let row_content = row![
        column![
            field_label("Launch at startup"),
            caption("Start Easy Harvest automatically when you log in."),
        ]
        .spacing(3)
        .width(Length::Fill),
        toggle,
    ]
    .align_y(Alignment::Center);

    container(row_content)
        .style(card_style)
        .padding(12)
        .width(Length::Fill)
        .into()
}

// ── Connection section ────────────────────────────────────────────────────────

fn connection_section(state: &EasyHarvest) -> Element<'_, Message> {
    let connected = !state.settings.account_id.is_empty();

    let status_row: Element<Message> = if connected {
        row![
            text(format!("Connected · Account {}", state.settings.account_id))
                .font(FONT_REGULAR)
                .size(12)
                .color(SUCCESS),
            Space::new().width(Length::Fill),
            button(
                text("Log out").font(FONT_MEDIUM).size(12).color(DANGER),
            )
            .style(outline_btn_style)
            .padding([5, 12])
            .on_press(Message::Settings(SettingsMsg::Disconnect)),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        Space::new().into()
    };

    container(
        column![
            section_heading("Connection"),
            status_row,
            field_label("Personal Access Token"),
            token_input(state),
            field_label("Account ID"),
            account_input(state),
            connect_btn(state),
            connection_error(state),
        ]
        .spacing(SECTION_GAP),
    )
    .style(card_style)
    .padding(12)
    .width(Length::Fill)
    .into()
}

// ── Shared field widgets ──────────────────────────────────────────────────────

fn token_input(state: &EasyHarvest) -> Element<'_, Message> {
    text_input("Paste your token here…", &state.settings_form.token_input)
        .on_input(|v| Message::Settings(SettingsMsg::TokenChanged(v)))
        .secure(true)
        .size(13)
        .padding([8, 10])
        .style(input_style)
        .into()
}

fn account_input(state: &EasyHarvest) -> Element<'_, Message> {
    text_input("e.g. 123456", &state.settings_form.account_input)
        .on_input(|v| Message::Settings(SettingsMsg::AccountIdChanged(v)))
        .size(13)
        .padding([8, 10])
        .style(input_style)
        .into()
}

fn connect_btn(state: &EasyHarvest) -> Element<'_, Message> {
    let label = if state.settings_form.connecting { "Connecting…" } else { "Connect" };
    let b = primary_btn(label)
        .width(Length::Fill);

    if state.settings_form.connecting { b.into() } else { b.on_press(Message::Settings(SettingsMsg::Save)).into() }
}

fn connection_error(state: &EasyHarvest) -> Element<'_, Message> {
    if let Some(err) = &state.settings_form.error {
        text(err).font(FONT_REGULAR).size(12).color(DANGER).into()
    } else {
        Space::new().into()
    }
}

// ── Layout helpers ────────────────────────────────────────────────────────────

/// A label-fill | input(width=72) | unit row with consistent alignment.
fn numeric_row<'a>(
    label: &'a str,
    value: &'a str,
    placeholder: &'a str,
    msg: impl Fn(String) -> Message + 'a,
    unit: &'a str,
) -> Element<'a, Message> {
    row![
        text(label)
            .font(FONT_REGULAR)
            .size(13)
            .color(TEXT_MUTED)
            .width(Length::Fill),
        text_input(placeholder, value)
            .on_input(msg)
            .size(13)
            .padding([7, 8])
            .style(input_style)
            .width(72),
        Space::new().width(8).height(8),
        text(unit)
            .font(FONT_REGULAR)
            .size(12)
            .color(TEXT_MUTED)
            .width(84),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn first_work_day_row(state: &EasyHarvest) -> Element<'_, Message> {
    row![
        column![
            text("First work day")
                .font(FONT_REGULAR)
                .size(13)
                .color(TEXT_MUTED),
            text("Optional — adjusts your vacation days for your first year if you did not start on January 1st.")
                .font(FONT_REGULAR)
                .size(11)
                .color(TEXT_MUTED),
        ]
        .spacing(1)
        .width(Length::Fill),
        text_input("DD.MM.YYYY", &state.settings_form.first_work_day_input)
            .on_input(|v| Message::Settings(SettingsMsg::FirstWorkDayChanged(v)))
            .size(13)
            .padding([7, 8])
            .style(input_style)
            .width(100),
        Space::new().width(92), // aligns with the unit label in numeric_row (8px gap + 84px unit)
    ]
    .align_y(Alignment::Center)
    .into()
}

fn template_add_form(state: &EasyHarvest) -> Element<'_, Message> {
    use crate::state::favorites::ProjectOption;

    let opts = &state.cached_project_options;
    let query = state.template_form.project_query.to_lowercase();
    let suggestions: Vec<(usize, &ProjectOption)> = opts
        .iter()
        .enumerate()
        .filter(|(_, o)| {
            state.template_form.project_idx.is_none()
                && o.matches_query(&query)
                && !state.template_form.project_query.is_empty()
        })
        .take(6)
        .collect();

    let project_suggestions: Element<Message> = if !suggestions.is_empty() {
        let items: Vec<Element<Message>> = suggestions
            .iter()
            .map(|(idx, opt)| {
                let idx = *idx;
                button(
                    text(opt.search_text.clone())
                        .font(FONT_REGULAR)
                        .size(13)
                        .color(TEXT_PRIMARY),
                )
                .style(super::suggestion_btn_style)
                .padding([8, 12])
                .width(Length::Fill)
                .on_press(Message::Settings(SettingsMsg::TemplateAddProjectSelected(idx)))
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

    let error: Element<Message> = if let Some(err) = &state.template_form.error {
        text(err.as_str()).font(FONT_REGULAR).size(12).color(DANGER).into()
    } else {
        Space::new().into()
    };

    let save_btn = primary_btn("Save Template")
        .on_press(Message::Settings(SettingsMsg::TemplateAddSave));

    column![
        field_label("Name"),
        text_input("e.g. Travel Luzern-Olten", &state.template_form.label)
            .on_input(|v| Message::Settings(SettingsMsg::TemplateAddLabelChanged(v)))
            .size(13)
            .padding([8, 10])
            .style(input_style),
        field_label("Project & Task"),
        text_input("Search…", &state.template_form.project_query)
            .on_input(|v| Message::Settings(SettingsMsg::TemplateAddProjectQueryChanged(v)))
            .size(13)
            .padding([8, 10])
            .style(input_style),
        project_suggestions,
        field_label("Default hours (optional)"),
        text_input("e.g. 1:30", &state.template_form.hours)
            .on_input(|v| Message::Settings(SettingsMsg::TemplateAddHoursChanged(v)))
            .size(13)
            .padding([8, 10])
            .style(input_style),
        field_label("Notes (optional)"),
        text_input("Pre-filled notes…", &state.template_form.notes)
            .on_input(|v| Message::Settings(SettingsMsg::TemplateAddNotesChanged(v)))
            .size(13)
            .padding([8, 10])
            .style(input_style),
        error,
        save_btn,
    ]
    .spacing(6)
    .into()
}


// ── Entry templates section ───────────────────────────────────────────────────

fn templates_section(state: &EasyHarvest) -> Element<'_, Message> {
    let header_btn: Element<Message> = if state.template_form.open {
        button(text("Cancel").font(FONT_MEDIUM).size(12).color(TEXT_MUTED))
            .style(|_: &iced::Theme, _| button::Style {
                background: None,
                text_color: TEXT_MUTED,
                ..Default::default()
            })
            .padding([4, 8])
            .on_press(Message::Settings(SettingsMsg::TemplateAddCancel))
            .into()
    } else {
        button(text("+ Add").font(FONT_MEDIUM).size(12).color(ACCENT))
            .style(|_: &iced::Theme, status| button::Style {
                background: Some(iced::Background::Color(match status {
                    button::Status::Hovered => with_alpha(ACCENT, 0.10),
                    _ => Color::TRANSPARENT,
                })),
                text_color: ACCENT,
                border: iced::Border {
                    color: with_alpha(ACCENT, 0.35),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..Default::default()
            })
            .padding([4, 10])
            .on_press(Message::Settings(SettingsMsg::TemplateAddOpen))
            .into()
    };

    let heading_row = row![
        section_heading("Entry Templates"),
        Space::new().width(Length::Fill),
        header_btn,
    ]
    .align_y(Alignment::Center);

    // ── Existing templates ────────────────────────────────────────────────────

    let template_rows: Vec<Element<Message>> = state
        .templates
        .entries
        .iter()
        .enumerate()
        .map(|(idx, tpl)| {
            let hours_label: Element<Message> = if tpl.hours.is_empty() {
                Space::new().into()
            } else {
                caption(format!("{}h", tpl.hours))
                    .into()
            };

            let notes_label: Element<Message> = if tpl.notes.is_empty() {
                Space::new().into()
            } else {
                caption(&tpl.notes)
                    .into()
            };

            // Look up project/task names for display
            let opts = &state.cached_project_options;
            let task_label: Element<Message> = if let Some(opt) = opts.iter().find(|o| {
                o.project_id == tpl.project_id && o.task_id == tpl.task_id
            }) {
                caption(format!("{} › {}", opt.project_name, opt.task_name))
                    .into()
            } else {
                Space::new().into()
            };

            let del_btn = super::delete_chip_btn(Message::Settings(SettingsMsg::TemplateDelete(idx)));

            row![
                column![
                    text(&tpl.label).font(FONT_MEDIUM).size(13).color(TEXT_PRIMARY),
                    task_label,
                    row![hours_label, notes_label].spacing(8),
                ]
                .spacing(2)
                .width(Length::Fill),
                del_btn,
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    let empty_hint: Element<Message> = if state.templates.entries.is_empty() && !state.template_form.open {
        text("No templates yet. Add one to quickly fill the entry form.")
            .font(FONT_REGULAR)
            .size(12)
            .color(TEXT_MUTED)
            .into()
    } else {
        Space::new().into()
    };

    // ── Add form ──────────────────────────────────────────────────────────────

    let add_form: Element<Message> = if state.template_form.open {
        template_add_form(state)
    } else {
        Space::new().into()
    };

    let template_list: Element<Message> = if template_rows.is_empty() {
        Space::new().into()
    } else {
        column(template_rows).spacing(LIST_ROW_SPACING).into()
    };

    container(
        column![heading_row, empty_hint, template_list, add_form]
            .spacing(SECTION_GAP),
    )
        .style(card_style)
        .padding(12)
        .width(Length::Fill)
        .into()
}

// end of settings_view
