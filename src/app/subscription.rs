use super::*;

// ── Subscription ──────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        let tick = if self.window_visible {
            iced::time::every(std::time::Duration::from_secs(30))
                .map(|_| Message::WorkDay(WorkDayMsg::Tick))
        } else {
            Subscription::none()
        };

        let tab = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed { key: keyboard::Key::Named(keyboard::key::Named::Tab), modifiers, .. } => {
                Some(Message::TabPressed { shift: modifiers.shift() })
            }
            _ => None,
        });

        // Moving the window between monitors with different DPI/scaling can
        // leave the render surface at a stale size (blurry, stretched UI).
        let rescaled = window::events().filter_map(|(id, event)| match event {
            window::Event::Rescaled(_) => Some(Message::WindowRescaled(id)),
            _ => None,
        });

        // Card drag on the Projects page: release drops, Escape cancels.
        let drag = if self.project_tracking.drag.is_some() {
            iced::event::listen_with(drag_event)
        } else {
            Subscription::none()
        };

        #[cfg(not(target_os = "macos"))]
        {
            let close = window::close_requests().map(Message::WindowCloseRequested);
            Subscription::batch([
                tick,
                tab,
                rescaled,
                drag,
                close,
                crate::tray::tray_subscription(self.tray_phase.clone(), self.tray_update_notify.clone()),
            ])
        }
        #[cfg(target_os = "macos")]
        {
            Subscription::batch([tick, tab, rescaled, drag])
        }
    }
}

fn drag_event(event: iced::Event, _status: iced::event::Status, _window: window::Id) -> Option<Message> {
    let msg = match event {
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            ProjectTrackingMsg::DragDrop
        }
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) => ProjectTrackingMsg::DragCancel,
        _ => return None,
    };
    Some(Message::ProjectTracking(msg))
}
