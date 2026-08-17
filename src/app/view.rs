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
        };

        let nav = nav_bar(&self.page);

        let mut col = column![nav].spacing(0).height(iced::Length::Fill);

        if let Some(err) = &self.error_banner {
            col = col.push(error_banner(err));
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

fn nav_bar(current: &Page) -> Element<'static, Message> {
    let btn = |label: &'static str, page: Page, active: bool| {
        let style = if active {
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
        .on_press(Message::Nav(NavMsg::PageChanged(page)))
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

    container(
        row![
            btn("Day", Page::Day, *current == Page::Day),
            btn("Vacation", Page::Vacation, *current == Page::Vacation),
            btn("Overtime", Page::Stats, *current == Page::Stats),
            btn("Billable", Page::Billable, *current == Page::Billable),
            btn("Projects", Page::ProjectTracking, *current == Page::ProjectTracking),
            Space::new().width(iced::Length::Fill),
            settings_btn,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .style(|_| container::Style {
        background: Some(iced::Background::Color(SURFACE)),
        ..Default::default()
    })
    .padding([10, 16])
    .width(iced::Length::Fill)
    .into()
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
        UpdateState::Verifying { .. } => (
            "Verifying update…".to_string(),
            Some(update_button_el("Verifying…", None)),
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
