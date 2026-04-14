use super::*;

// ── Subscription ──────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        let tick = if self.window_visible {
            iced::time::every(std::time::Duration::from_secs(30))
                .map(|_| Message::WorkDayTick)
        } else {
            Subscription::none()
        };

        let tab = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => match key {
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    Some(Message::TabPressed { shift: modifiers.shift() })
                }
                _ => None,
            },
            _ => None,
        });

        #[cfg(not(target_os = "macos"))]
        {
            let close = window::close_requests().map(Message::WindowCloseRequested);
            Subscription::batch([
                tick,
                tab,
                close,
                crate::tray::tray_subscription(self.tray_phase.clone(), self.tray_update_notify.clone()),
            ])
        }
        #[cfg(target_os = "macos")]
        {
            Subscription::batch([tick, tab])
        }
    }
}
