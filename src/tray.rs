use std::sync::{Arc, Mutex};

use iced::Subscription;
use ksni::{menu::StandardItem, TrayMethods};
use tokio::sync::{mpsc, Notify};

use crate::state::work_day::WorkPhase;

// Pre-computed ARGB32 pixel data generated at build time by build.rs.
// No runtime image decoding — icon_pixmap() is just a slice copy.
const TRAY_16: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_16.argb32"));
const TRAY_22: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_22.argb32"));
const TRAY_32: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_32.argb32"));

#[derive(Debug, Clone)]
pub enum TrayAction {
    ToggleWindow,
    StartDay,
    StartBreak,
    EndBreak,
    EndDay,
    ResumeDay,
    Quit,
}

struct EasyHarvestTray {
    sender: mpsc::Sender<TrayAction>,
    phase: Arc<Mutex<WorkPhase>>,
}

impl ksni::Tray for EasyHarvestTray {
    fn id(&self) -> String {
        "easy_harvest".to_string()
    }

    fn title(&self) -> String {
        "Easy Harvest".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![
            ksni::Icon { width: 16, height: 16, data: TRAY_16.to_vec() },
            ksni::Icon { width: 22, height: 22, data: TRAY_22.to_vec() },
            ksni::Icon { width: 32, height: 32, data: TRAY_32.to_vec() },
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.try_send(TrayAction::ToggleWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let phase = self.phase.lock().map(|p| *p).unwrap_or_default();

        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label: "Open Easy Harvest".to_string(),
                activate: Box::new(|this: &mut EasyHarvestTray| {
                    let _ = this.sender.try_send(TrayAction::ToggleWindow);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
        ];

        match phase {
            WorkPhase::NotStarted => {
                items.push(
                    StandardItem {
                        label: "Start Day".to_string(),
                        activate: Box::new(|this: &mut EasyHarvestTray| {
                            let _ = this.sender.try_send(TrayAction::StartDay);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            WorkPhase::Working => {
                items.push(
                    StandardItem {
                        label: "Start Break".to_string(),
                        activate: Box::new(|this: &mut EasyHarvestTray| {
                            let _ = this.sender.try_send(TrayAction::StartBreak);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
                items.push(
                    StandardItem {
                        label: "End Day".to_string(),
                        activate: Box::new(|this: &mut EasyHarvestTray| {
                            let _ = this.sender.try_send(TrayAction::EndDay);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            WorkPhase::OnBreak => {
                items.push(
                    StandardItem {
                        label: "End Break".to_string(),
                        activate: Box::new(|this: &mut EasyHarvestTray| {
                            let _ = this.sender.try_send(TrayAction::EndBreak);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            WorkPhase::Ended => {
                items.push(
                    StandardItem {
                        label: "Resume Day".to_string(),
                        activate: Box::new(|this: &mut EasyHarvestTray| {
                            let _ = this.sender.try_send(TrayAction::ResumeDay);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
        }

        items.push(ksni::MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Quit Easy Harvest".to_string(),
                activate: Box::new(|this: &mut EasyHarvestTray| {
                    let _ = this.sender.try_send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

enum TrayState {
    Bootstrap(Arc<Mutex<WorkPhase>>, Arc<Notify>),
    Active(mpsc::Receiver<TrayAction>, ksni::Handle<EasyHarvestTray>, Arc<Notify>),
    Failed,
}

pub fn tray_subscription(
    phase: Arc<Mutex<WorkPhase>>,
    update_notify: Arc<Notify>,
) -> Subscription<crate::app::Message> {
    use crate::app::Message;
    use iced::futures::stream;

    Subscription::run_with_id(
        "easy_harvest_tray",
        stream::unfold(TrayState::Bootstrap(phase, update_notify), |state| async move {
            match state {
                TrayState::Bootstrap(phase, notify) => {
                    let (tx, rx) = mpsc::channel::<TrayAction>(4);
                    let tray = EasyHarvestTray { sender: tx, phase };
                    match tray.spawn().await {
                        Ok(handle) => Some((Message::TrayReady, TrayState::Active(rx, handle, notify))),
                        Err(e) => {
                            eprintln!("Tray spawn failed: {e}");
                            Some((Message::TrayUnavailable, TrayState::Failed))
                        }
                    }
                }
                TrayState::Active(mut rx, handle, notify) => {
                    tokio::select! {
                        action = rx.recv() => match action {
                            Some(action) => {
                                let msg = match action {
                                    TrayAction::ToggleWindow => Message::TrayToggle,
                                    TrayAction::StartDay    => Message::StartDay,
                                    TrayAction::StartBreak  => Message::StartBreak,
                                    TrayAction::EndBreak    => Message::EndBreak,
                                    TrayAction::EndDay      => Message::EndDay,
                                    TrayAction::ResumeDay   => Message::ResumeDay,
                                    TrayAction::Quit        => Message::QuitApp,
                                };
                                Some((msg, TrayState::Active(rx, handle, notify)))
                            }
                            None => None,
                        },
                        _ = notify.notified() => {
                            handle.update(|_: &mut EasyHarvestTray| {}).await;
                            Some((Message::TrayMenuRefreshed, TrayState::Active(rx, handle, notify)))
                        }
                    }
                }
                TrayState::Failed => None,
            }
        }),
    )
}
