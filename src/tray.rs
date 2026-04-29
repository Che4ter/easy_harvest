use std::sync::{Arc, Mutex};

use iced::Subscription;
use iced::advanced::subscription::{self, EventStream};
use tokio::sync::Notify;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

use crate::state::work_day::WorkPhase;

// Pre-computed RGBA8 pixel data committed to assets/.
const TRAY_32: &[u8] = include_bytes!("../assets/tray_32.rgba8");

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

/// Commands sent from the iced subscription to the OS tray thread.
enum TrayCommand {
    UpdatePhase(WorkPhase),
}

/// Identifiers for the menu items.  Each phase-dependent action gets an id so
/// we can match incoming `MenuEvent` back to the correct `TrayAction`.
struct MenuIds {
    toggle: tray_icon::menu::MenuId,
    start_day: tray_icon::menu::MenuId,
    start_break: tray_icon::menu::MenuId,
    end_break: tray_icon::menu::MenuId,
    end_day: tray_icon::menu::MenuId,
    resume_day: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

/// Build a context menu for the given work-phase.  Returns the `Menu` and the
/// set of `MenuIds` that the event handler needs to decode clicks.
fn build_menu(phase: WorkPhase) -> (Menu, MenuIds) {
    let toggle = MenuItem::new("Open Easy Harvest", true, None);
    let start_day = MenuItem::new("Start Day", true, None);
    let start_break = MenuItem::new("Start Break", true, None);
    let end_break = MenuItem::new("End Break", true, None);
    let end_day = MenuItem::new("End Day", true, None);
    let resume_day = MenuItem::new("Resume Day", true, None);
    let quit = MenuItem::new("Quit Easy Harvest", true, None);

    let ids = MenuIds {
        toggle: toggle.id().clone(),
        start_day: start_day.id().clone(),
        start_break: start_break.id().clone(),
        end_break: end_break.id().clone(),
        end_day: end_day.id().clone(),
        resume_day: resume_day.id().clone(),
        quit: quit.id().clone(),
    };

    let menu = Menu::new();
    let _ = menu.append(&toggle);
    let _ = menu.append(&PredefinedMenuItem::separator());

    match phase {
        WorkPhase::NotStarted => {
            let _ = menu.append(&start_day);
        }
        WorkPhase::Working => {
            let _ = menu.append(&start_break);
            let _ = menu.append(&end_day);
        }
        WorkPhase::OnBreak => {
            let _ = menu.append(&end_break);
        }
        WorkPhase::Ended => {
            let _ = menu.append(&resume_day);
        }
    }

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    (menu, ids)
}

fn map_menu_event(id: &tray_icon::menu::MenuId, ids: &MenuIds) -> Option<TrayAction> {
    if id == &ids.toggle {
        Some(TrayAction::ToggleWindow)
    } else if id == &ids.start_day {
        Some(TrayAction::StartDay)
    } else if id == &ids.start_break {
        Some(TrayAction::StartBreak)
    } else if id == &ids.end_break {
        Some(TrayAction::EndBreak)
    } else if id == &ids.end_day {
        Some(TrayAction::EndDay)
    } else if id == &ids.resume_day {
        Some(TrayAction::ResumeDay)
    } else if id == &ids.quit {
        Some(TrayAction::Quit)
    } else {
        None
    }
}

/// Spawn the OS thread that owns the `TrayIcon`.
///
/// Returns a channel to receive tray actions from the menu, and a channel to
/// send phase-update commands to the tray thread.
fn spawn_tray_thread(
    initial_phase: WorkPhase,
) -> Result<
    (
        std::sync::mpsc::Receiver<TrayAction>,
        std::sync::mpsc::Sender<TrayCommand>,
    ),
    String,
> {
    let (action_tx, action_rx) = std::sync::mpsc::channel::<TrayAction>();
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<TrayCommand>();

    // One-shot channel so the spawned thread can report success or failure.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

    std::thread::Builder::new()
        .name("tray-icon".into())
        .spawn(move || {
            // On Linux, tray-icon uses libappindicator which requires GTK.
            // Initialize GTK on this thread so the tray can render.
            #[cfg(target_os = "linux")]
            if gtk::init().is_err() {
                let _ = ready_tx.send(Err("GTK init failed".into()));
                return;
            }

            // Build initial menu and icon.
            let icon = match Icon::from_rgba(TRAY_32.to_vec(), 32, 32) {
                Ok(i) => i,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Icon error: {e}")));
                    return;
                }
            };

            let (menu, mut ids) = build_menu(initial_phase);

            let _tray = match TrayIconBuilder::new()
                .with_tooltip("Easy Harvest")
                .with_icon(icon)
                .with_menu(Box::new(menu))
                .build()
            {
                Ok(t) => t,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("TrayIcon build error: {e}")));
                    return;
                }
            };

            let _ = ready_tx.send(Ok(()));

            // Event loop: poll menu events, icon clicks, and incoming commands.
            let menu_rx = MenuEvent::receiver();
            let icon_rx = TrayIconEvent::receiver();
            loop {
                // Check for menu clicks (non-blocking).
                while let Ok(event) = menu_rx.try_recv() {
                    if let Some(action) = map_menu_event(&event.id, &ids)
                        && action_tx.send(action).is_err() {
                            return; // subscription dropped
                        }
                }

                // Check for tray icon clicks — left-click toggles the window.
                // Do NOT match DoubleClick: on Windows a double-click fires
                // both Click and DoubleClick, causing ToggleWindow twice
                // (open then immediately close).
                while let Ok(event) = icon_rx.try_recv() {
                    if matches!(
                        &event,
                        TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. }
                    ) && action_tx.send(TrayAction::ToggleWindow).is_err()
                    {
                        return;
                    }
                }

                // Check for phase-update commands (non-blocking).
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TrayCommand::UpdatePhase(phase) => {
                            let (new_menu, new_ids) = build_menu(phase);
                            _tray.set_menu(Some(Box::new(new_menu)));
                            ids = new_ids;
                        }
                    }
                }

                // Pump the platform event loop so the tray icon's hidden
                // window can process WM_COMMAND (menu clicks) and tray
                // notification messages.
                #[cfg(target_os = "linux")]
                gtk::main_iteration_do(false);
                #[cfg(target_os = "windows")]
                {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
                    };
                    // Drain all pending messages before sleeping.
                    unsafe {
                        let mut msg = std::mem::zeroed::<MSG>();
                        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        })
        .map_err(|e| format!("Thread spawn error: {e}"))?;

    // Wait for the tray to initialise (with a timeout).
    match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => Ok((action_rx, cmd_tx)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Tray init timed out".into()),
    }
}

enum TrayState {
    Bootstrap(Arc<Mutex<WorkPhase>>, Arc<Notify>),
    Active {
        action_rx: std::sync::mpsc::Receiver<TrayAction>,
        cmd_tx: std::sync::mpsc::Sender<TrayCommand>,
        notify: Arc<Notify>,
        phase: Arc<Mutex<WorkPhase>>,
    },
    Failed,
}

struct TrayRecipe {
    phase: Arc<Mutex<WorkPhase>>,
    update_notify: Arc<Notify>,
}

impl subscription::Recipe for TrayRecipe {
    type Output = crate::app::Message;

    fn hash(&self, state: &mut subscription::Hasher) {
        use std::hash::Hash;
        "easy_harvest_tray".hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: EventStream,
    ) -> iced::futures::stream::BoxStream<'static, Self::Output> {
        use crate::app::Message;
        use iced::futures::stream;

        Box::pin(stream::unfold(
            TrayState::Bootstrap(self.phase, self.update_notify),
            |state| async move {
                match state {
                    TrayState::Bootstrap(phase, notify) => {
                        let initial_phase = phase.lock().map(|p| *p).unwrap_or_default();
                        match spawn_tray_thread(initial_phase) {
                            Ok((action_rx, cmd_tx)) => Some((
                                Message::TrayReady,
                                TrayState::Active { action_rx, cmd_tx, notify, phase },
                            )),
                            Err(e) => {
                                eprintln!("Tray spawn failed: {e}");
                                Some((Message::TrayUnavailable, TrayState::Failed))
                            }
                        }
                    }
                    TrayState::Active { action_rx, cmd_tx, notify, phase } => {
                        // Use tokio::select! to wait for either a menu action
                        // or a notify signal to update the phase.
                        loop {
                            // First, drain any pending actions.
                            if let Ok(action) = action_rx.try_recv() {
                                let msg = match action {
                                    TrayAction::ToggleWindow => Message::TrayToggle,
                                    TrayAction::StartDay => Message::WorkDay(crate::app::WorkDayMsg::Start),
                                    TrayAction::StartBreak => Message::WorkDay(crate::app::WorkDayMsg::StartBreak),
                                    TrayAction::EndBreak => Message::WorkDay(crate::app::WorkDayMsg::EndBreak),
                                    TrayAction::EndDay => Message::WorkDay(crate::app::WorkDayMsg::End),
                                    TrayAction::ResumeDay => Message::WorkDay(crate::app::WorkDayMsg::Resume),
                                    TrayAction::Quit => Message::QuitApp,
                                };
                                return Some((msg, TrayState::Active { action_rx, cmd_tx, notify, phase }));
                            }

                            // Wait for either a notify signal or a short timeout
                            // to check the action channel again.
                            tokio::select! {
                                _ = notify.notified() => {
                                    let new_phase = phase.lock().map(|p| *p).unwrap_or_default();
                                    let _ = cmd_tx.send(TrayCommand::UpdatePhase(new_phase));
                                    return Some((
                                        Message::TrayMenuRefreshed,
                                        TrayState::Active { action_rx, cmd_tx, notify, phase },
                                    ));
                                }
                                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                                    // Loop back to check action_rx again.
                                }
                            }
                        }
                    }
                    TrayState::Failed => None,
                }
            },
        ))
    }
}

pub fn tray_subscription(
    phase: Arc<Mutex<WorkPhase>>,
    update_notify: Arc<Notify>,
) -> Subscription<crate::app::Message> {
    iced::advanced::subscription::from_recipe(TrayRecipe { phase, update_notify })
}
