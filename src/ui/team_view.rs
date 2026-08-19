use chrono::{Datelike, Local};
use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};

use crate::app::{
    EasyHarvest, Message, TeamMsg, DANGER, FONT_REGULAR, FONT_SEMIBOLD, SUCCESS, TEXT_PRIMARY,
    WARNING,
};
use super::{caption, card_style, outline_btn_sm, section_heading, stat_chip, PAGE_PADDING, SECTION_GAP};

pub fn view(state: &EasyHarvest) -> Element<'_, Message> {
    let year = Local::now().naive_local().date().year();

    if state.team_settings.members.is_empty() {
        return scrollable(
            column![
                section_heading("Team"),
                caption(
                    "No team members yet. Add teammates in Settings → Team \
                     to see their overtime and vacation here.",
                ),
            ]
            .spacing(SECTION_GAP)
            .padding(PAGE_PADDING),
        )
        .height(Length::Fill)
        .into();
    }

    let cards: Vec<Element<Message>> = state
        .team_settings
        .members
        .iter()
        .map(|m| member_card(state, m, year))
        .collect();

    scrollable(
        column![
            row![
                section_heading("Team"),
                Space::new().width(Length::Fill),
                outline_btn_sm("↻ Refresh").on_press(Message::Team(TeamMsg::Refresh)),
            ]
            .align_y(Alignment::Center),
            column(cards).spacing(SECTION_GAP),
        ]
        .spacing(SECTION_GAP)
        .padding(PAGE_PADDING),
    )
    .height(Length::Fill)
    .into()
}

fn member_card<'a>(
    state: &'a EasyHarvest,
    member: &'a crate::state::team::TeamMember,
    year: i32,
) -> Element<'a, Message> {
    let stats = state.team.stats.get(&member.harvest_user_id);

    let warn: Element<Message> = if member.missing_carryover_for(year) {
        text("⚠").size(14).color(WARNING).into()
    } else {
        Space::new().into()
    };

    let name_row = row![
        text(member.display_name.clone())
            .font(FONT_SEMIBOLD)
            .size(15)
            .color(TEXT_PRIMARY),
        warn,
        Space::new().width(Length::Fill),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let body: Element<Message> = match stats {
        None => caption("Not loaded yet — press Refresh.").into(),
        Some(s) if s.loading => caption("Loading…").into(),
        Some(s) => {
            if let Some(err) = &s.error {
                text(err.clone()).font(FONT_REGULAR).size(12).color(DANGER).into()
            } else {
                let balance = s.year_balance.as_ref();
                let holidays = s.holiday_stats.as_ref();
                let total_balance = balance.map(|b| b.total_balance).unwrap_or(0.0);
                let ot_color = if total_balance >= 0.0 { SUCCESS } else { DANGER };
                row![
                    stat_chip(
                        "Overtime",
                        format!("{total_balance:+.1}h"),
                        String::new(),
                        ot_color,
                    ),
                    stat_chip(
                        "Vacation taken",
                        format!("{:.1}d", holidays.map(|h| h.days_taken).unwrap_or(0.0)),
                        String::new(),
                        TEXT_PRIMARY,
                    ),
                    stat_chip(
                        "Vacation remaining",
                        format!("{:.1}d", holidays.map(|h| h.days_remaining).unwrap_or(0.0)),
                        String::new(),
                        TEXT_PRIMARY,
                    ),
                ]
                .spacing(8)
                .into()
            }
        }
    };

    let impersonate_btn = outline_btn_sm("Impersonate")
        .on_press(Message::Team(TeamMsg::ImpersonationStart(member.harvest_user_id)));

    let actions_row = row![Space::new().width(Length::Fill), impersonate_btn]
        .align_y(Alignment::Center);

    container(column![name_row, body, actions_row].spacing(10))
        .style(card_style)
        .padding(12)
        .width(Length::Fill)
        .into()
}
