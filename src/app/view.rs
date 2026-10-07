use super::*;
// Re-import `column` explicitly to avoid ambiguity with `std::column!` macro.
use iced::widget::column;

// ── View ──────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let content: Element<Message> = match &self.page {
            Page::Settings => settings_view::view(self),
            Page::Day => day_view::view(self),
            Page::Stats => stats_view::view(self),
            Page::Vacation => vacation_view::view(self),
            Page::Billable => billable_view::view(self),
            Page::ProjectTracking => project_tracking_view::view(self),
            Page::Team => team_view::view(self),
        };

        let nav = nav_bar(&self.page, self.settings.team_lead_mode, self.impersonating.is_some());

        let mut col = column![nav].spacing(0).height(iced::Length::Fill);

        if let Some(id) = self.impersonating {
            let name = self.team_settings.member(id)
                .map(|m| m.display_name.clone())
                .unwrap_or_else(|| "member".to_string());
            col = col.push(impersonation_banner(&name));
        }

        if let Some(err) = &self.error_banner {
            col = col.push(error_banner(err));
        }

        if self.show_unsubmitted_banner() {
            col = col.push(unsubmitted_banner(self));
        }

        if let Some(banner) = update_banner(&self.update_state) {
            col = col.push(banner);
        }

        col = col.push(content);

        container(col)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(BACKGROUND)),
                ..Default::default()
            })
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }
}

// ── Nav bar ───────────────────────────────────────────────────────────────────

fn nav_bar(current: &Page, team_lead_mode: bool, impersonating: bool) -> Element<'static, Message> {
    let btn = |label: &'static str, page: Page, active: bool, disabled: bool| {
        let style = if disabled {
            button::Style {
                background: Some(iced::Background::Color(SURFACE_RAISED)),
                text_color: Color { a: 0.35, ..TEXT_MUTED },
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        } else if active {
            button::Style {
                background: Some(iced::Background::Color(ACCENT)),
                text_color: Color::WHITE,
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        } else {
            button::Style {
                background: Some(iced::Background::Color(SURFACE_RAISED)),
                text_color: TEXT_MUTED,
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        };
        button(
            text(label)
                .font(FONT_MEDIUM)
                .size(13),
        )
        .style(move |_, _| style)
        .padding([6, 14])
        .on_press_maybe((!disabled).then_some(Message::Nav(NavMsg::PageChanged(page))))
    };

    let settings_active = *current == Page::Settings;
    let settings_btn = button(
        text("Settings").font(FONT_MEDIUM).size(13),
    )
    .style(move |_, _| {
        if settings_active {
            button::Style {
                background: Some(iced::Background::Color(ACCENT)),
                text_color: Color::WHITE,
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }
        } else {
            button::Style {
                background: Some(iced::Background::Color(SURFACE_RAISED)),
                text_color: TEXT_MUTED,
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }
        }
    })
    .padding([6, 12])
    .on_press(Message::Nav(NavMsg::PageChanged(Page::Settings)));

    let mut nav_row = row![
        btn("Day", Page::Day, *current == Page::Day, false),
        btn("Vacation", Page::Vacation, *current == Page::Vacation, false),
        btn("Overtime", Page::Stats, *current == Page::Stats, false),
        btn("Billable", Page::Billable, *current == Page::Billable, impersonating),
        btn("Projects", Page::ProjectTracking, *current == Page::ProjectTracking, impersonating),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    if team_lead_mode {
        nav_row = nav_row.push(btn("Team", Page::Team, *current == Page::Team, false));
    }

    nav_row = nav_row.push(Space::new().width(iced::Length::Fill));
    nav_row = nav_row.push(settings_btn);

    container(nav_row)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(SURFACE)),
            ..Default::default()
        })
        .padding([10, 16])
        .width(iced::Length::Fill)
        .into()
}

/// Full-width coloured bar: message on the left, buttons on the right.
fn action_banner<'a>(
    message: String,
    background: Color,
    actions: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut content = row![
        text(message).font(FONT_REGULAR).size(13).color(Color::WHITE),
        Space::new().width(iced::Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    for action in actions {
        content = content.push(action);
    }
    container(content)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(background)),
            ..Default::default()
        })
        .padding([8, 16])
        .width(iced::Length::Fill)
        .into()
}

/// Translucent-dark button used inside coloured banners.
fn banner_btn(label: &str, msg: Message) -> Element<'_, Message> {
    button(text(label).font(FONT_MEDIUM).size(13).color(Color::WHITE))
        .style(|_, _| button::Style {
            background: Some(iced::Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.20 })),
            text_color: Color::WHITE,
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([4, 10])
        .on_press(msg)
        .into()
}

fn impersonation_banner(name: &str) -> Element<'static, Message> {
    action_banner(
        format!("Viewing {name} — read-only"),
        ACCENT,
        vec![banner_btn("Exit", Message::Team(TeamMsg::ImpersonationExit))],
    )
}

fn unsubmitted_banner(state: &EasyHarvest) -> Element<'_, Message> {
    let today = Local::now().naive_local().date();
    let mut actions = Vec::new();
    actions.push(banner_btn("Open in Harvest", Message::Unsubmitted(UnsubmittedMsg::OpenInHarvest)));
    actions.push(banner_btn("✕", Message::Unsubmitted(UnsubmittedMsg::Dismiss)));
    action_banner(
        crate::unsubmitted::banner_text(&state.unsubmitted_weeks, today),
        WARNING,
        actions,
    )
}

fn error_banner(msg: &str) -> Element<'_, Message> {
    container(
        text(msg).font(FONT_REGULAR).size(13).color(Color::WHITE),
    )
    .style(|_| container::Style {
        background: Some(iced::Background::Color(DANGER)),
        ..Default::default()
    })
    .padding([8, 16])
    .width(iced::Length::Fill)
    .into()
}

fn update_banner(state: &UpdateState) -> Option<Element<'_, Message>> {
    let (status_text, update_button): (String, Option<Element<Message>>) = match state {
        UpdateState::Idle => return None,
        UpdateState::Available { tag, assets } => (
            format!("Update available: {tag}"),
            assets
                .as_ref()
                .map(|_| update_button_el("Update now", Some(Message::StartUpdate))),
        ),
        UpdateState::Downloading { .. } => (
            "Downloading update…".to_string(),
            Some(update_button_el("Downloading…", None)),
        ),
        UpdateState::Installing { .. } => (
            "Installing update…".to_string(),
            Some(update_button_el("Installing…", None)),
        ),
        UpdateState::Failed { reason, .. } => (
            format!("Update failed: {reason}"),
            Some(update_button_el("Update now", Some(Message::StartUpdate))),
        ),
        UpdateState::InstalledNeedsManualRestart(reason) => (
            format!(
                "Update installed but couldn't restart automatically: {reason}. Please launch Easy Harvest manually."
            ),
            None,
        ),
    };

    let link = button(
        text("View release →")
            .font(FONT_MEDIUM)
            .size(13)
            .color(Color::WHITE),
    )
    .style(|_, _| button::Style {
        background: Some(iced::Background::Color(Color {
            r: 0.0, g: 0.0, b: 0.0, a: 0.20,
        })),
        text_color: Color::WHITE,
        border: iced::Border { radius: 4.0.into(), ..Default::default() },
        ..Default::default()
    })
    .padding([4, 10])
    .on_press(Message::OpenReleases);

    let mut actions = row![].spacing(8).align_y(iced::Alignment::Center);
    if let Some(btn) = update_button {
        actions = actions.push(btn);
    }
    actions = actions.push(link);

    Some(
        container(
            row![
                text(status_text).font(FONT_REGULAR).size(13).color(Color::WHITE),
                Space::new().width(iced::Length::Fill),
                actions,
            ]
            .align_y(iced::Alignment::Center),
        )
        .style(|_| container::Style {
            background: Some(iced::Background::Color(ACCENT)),
            ..Default::default()
        })
        .padding([6, 16])
        .width(iced::Length::Fill)
        .into(),
    )
}

fn update_button_el(label: &'static str, on_press: Option<Message>) -> Element<'static, Message> {
    let btn = button(text(label).font(FONT_MEDIUM).size(13).color(ACCENT))
        .style(|_, _| button::Style {
            background: Some(iced::Background::Color(Color {
                r: 1.0, g: 1.0, b: 1.0, a: 0.90,
            })),
            text_color: ACCENT,
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            ..Default::default()
        })
        .padding([4, 10]);
    match on_press {
        Some(msg) => btn.on_press(msg).into(),
        None => btn.into(),
    }
}
