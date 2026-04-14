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
