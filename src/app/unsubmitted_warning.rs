use std::time::{Duration, Instant};

use super::*;
use crate::unsubmitted::{last_completed_sunday, unsubmitted_weeks, week_url, normalize_web_address, UnsubmittedWeek};

/// Minimum gap between non-forced checks (window re-opened from the tray).
pub(super) const CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone)]
pub enum UnsubmittedMsg {
    /// Run the check unless throttled; `force` bypasses the throttle.
    Check { force: bool },
    Loaded(Result<Vec<UnsubmittedWeek>, String>),
    /// `base_uri` from `GET /v2/company`.
    CompanyLoaded(Result<String, String>),
    OpenInHarvest,
    /// Hide the banner until the app restarts.
    Dismiss,
}

impl EasyHarvest {
    /// Forget everything tied to the signed-in account, including the web
    /// address, so a different company gets its own banner and link.
    /// Caller persists settings.
    pub(super) fn reset_unsubmitted(&mut self) {
        self.unsubmitted_weeks.clear();
        self.unsubmitted_checked_at = None;
        self.unsubmitted_dismissed = false;
        self.settings.harvest_web_address = None;
        self.settings_form.web_address_input.clear();
    }

    pub fn show_unsubmitted_banner(&self) -> bool {
        !self.unsubmitted_dismissed && !self.unsubmitted_weeks.is_empty()
    }

    pub(super) fn should_check_unsubmitted(&self, force: bool, now: Instant) -> bool {
        force
            || self
                .unsubmitted_checked_at
                .is_none_or(|at| now.duration_since(at) >= CHECK_INTERVAL)
    }

    pub(super) fn update_unsubmitted(&mut self, msg: UnsubmittedMsg) -> Task<Message> {
        match msg {
            UnsubmittedMsg::Check { force } => {
                let now = Instant::now();
                if !self.should_check_unsubmitted(force, now) {
                    return Task::none();
                }
                // Always the signed-in user — impersonation does not apply.
                let (Some(client), Some(user_id)) = (self.client.clone(), self.harvest_user_id) else {
                    return Task::none();
                };
                self.unsubmitted_checked_at = Some(now);
                let today = Local::now().naive_local().date();
                let to = last_completed_sunday(today).format("%Y-%m-%d").to_string();
                Task::perform(
                    async move {
                        client
                            .list_unsubmitted_time_entries(user_id, &to)
                            .await
                            .map(|entries| unsubmitted_weeks(&entries, today))
                            .map_err(|e| e.to_string())
                    },
                    |r| Message::Unsubmitted(UnsubmittedMsg::Loaded(r)),
                )
            }
            UnsubmittedMsg::Loaded(Ok(weeks)) => {
                self.unsubmitted_weeks = weeks;
                Task::none()
            }
            UnsubmittedMsg::Loaded(Err(e)) => {
                eprintln!("Unsubmitted-weeks check failed: {e}");
                Task::none()
            }
            UnsubmittedMsg::CompanyLoaded(Ok(base_uri)) => {
                if self.settings.harvest_web_address.is_none()
                    && let Some(addr) = normalize_web_address(&base_uri)
                {
                    self.settings_form.web_address_input = addr.clone();
                    self.settings.harvest_web_address = Some(addr);
                    if let Err(e) = self.settings.save() {
                        eprintln!("Failed to save Harvest web address: {e}");
                    }
                }
                Task::none()
            }
            UnsubmittedMsg::CompanyLoaded(Err(e)) => {
                eprintln!("Company lookup failed: {e}");
                Task::none()
            }
            UnsubmittedMsg::OpenInHarvest => {
                if let (Some(base), Some(oldest)) =
                    (&self.settings.harvest_web_address, self.unsubmitted_weeks.first())
                {
                    let _ = open::that_detached(week_url(base, oldest.monday));
                }
                Task::none()
            }
            UnsubmittedMsg::Dismiss => {
                self.unsubmitted_dismissed = true;
                Task::none()
            }
        }
    }

    /// Admins without a configured web address: look it up once.
    pub(super) fn load_company_task(&self) -> Task<Message> {
        if !self.harvest_user_is_admin || self.settings.harvest_web_address.is_some() {
            return Task::none();
        }
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                client.get_company().await.map(|c| c.base_uri).map_err(|e| e.to_string())
            },
            |r| Message::Unsubmitted(UnsubmittedMsg::CompanyLoaded(r)),
        )
    }
}
