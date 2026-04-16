use super::*;

// ── Date picker state ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct DatePickerState {
    pub open: bool,
    pub month: NaiveDate,
}

impl DatePickerState {
    pub fn new(month: NaiveDate) -> Self {
        Self { open: false, month }
    }
}

// ── NavMsg ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum NavMsg {
    PageChanged(Page),
    DatePrev,
    DateNext,
    DateToday,
    DatePickerToggle,
    DatePickerMonthPrev,
    DatePickerMonthNext,
    DatePickerSelect(NaiveDate),
}

// ── Update ────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ── Navigation / Date ──
            Message::Nav(msg) => self.update_navigation(msg),

            // ── Entries / Timer ──
            Message::Entry(msg) => self.update_entries(*msg),

            // ── Work Day ──
            Message::WorkDay(msg) => self.update_work_day(msg),

            // ── Settings ──
            Message::Settings(msg) => self.update_settings(msg),

            // ── Vacation ──
            Message::Vacation(msg) => self.update_vacation(msg),

            // ── Billable ──
            Message::Billable(msg) => self.update_billable(msg),

            // ── Project Tracking ──
            Message::ProjectTracking(msg) => self.update_project_tracking(msg),

            // ── Stats ──
            Message::Stats(msg) => self.update_stats(msg),

            // ── Small inline arms ──
            Message::FontLoaded(_) => Task::none(),

            Message::TabPressed { shift } => {
                if shift {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                }
            }

            Message::TrayReady => {
                self.tray_available = true;
                Task::none()
            }

            Message::TrayMenuRefreshed => Task::none(),

            Message::TrayUnavailable => {
                self.tray_available = false;
                // The window may have already been closed under the assumption that
                // the tray was working.  Re-open it so the app remains reachable.
                if self.window_id.is_none() {
                    let (new_id, open_task) = window::open(window_settings());
                    self.window_id = Some(new_id);
                    self.window_visible = true;
                    open_task.map(|id| Message::WindowIdReceived(Some(id)))
                } else {
                    Task::none()
                }
            }

            Message::QuitApp => iced::exit(),

            Message::TrayToggle => {
                if let Some(id) = self.window_id {
                    // Window exists — bring it to front.
                    window::gain_focus(id)
                } else {
                    // Window was closed — reopen it.
                    let (new_id, open_task) = window::open(window_settings());
                    self.window_id = Some(new_id);
                    self.window_visible = true;
                    open_task.map(|id| Message::WindowIdReceived(Some(id)))
                }
            }

            Message::WindowIdReceived(id) => {
                self.window_id = id;
                Task::none()
            }

            Message::WindowCloseRequested(id) => {
                if self.tray_available {
                    // Close the window but keep the process alive (tray stays).
                    self.window_id = None;
                    self.window_visible = false;
                    window::close(id)
                } else {
                    iced::exit()
                }
            }
        }
    }

    // ── Navigation / Date ────────────────────────────────────────────────────

    fn update_navigation(&mut self, msg: NavMsg) -> Task<Message> {
        match msg {
            NavMsg::PageChanged(page) => {
                self.entry_form = None;
                self.error_banner = None;
                let task = match &page {
                    Page::Day => {
                        self.loading = true;
                        self.entries_gen += 1;
                        Task::batch([
                            self.load_entries_task(),
                            self.load_assignments_task(),
                        ])
                    }
                    Page::Stats => {
                        self.loading = true;
                        self.stats_gen += 1;
                        self.load_stats_task()
                    }
                    Page::Settings => {
                        // Refresh profile inputs from current settings
                        self.settings_form.weekly_hours_input = self.settings.total_weekly_hours.to_string();
                        self.settings_form.percentage_input = format!("{:.1}", self.settings.work_percentage * 100.0);
                        self.settings_form.holidays_input = self.settings.total_holiday_days_per_year.to_string();
                        self.settings_form.first_work_day_input = self.settings.first_work_day
                            .map(|d| d.format("%d.%m.%Y").to_string())
                            .unwrap_or_default();
                        self.settings_form.profile_saved = false;
                        self.settings_form.data_dir_input = self.settings.data_dir.display().to_string();
                        self.settings_form.data_dir_saved = false;
                        Task::none()
                    }
                    Page::Vacation => {
                        if self.vacation.entries.is_empty() && self.client.is_some() {
                            self.loading = true;
                            self.vacation_gen += 1;
                            self.load_vacation_task()
                        } else {
                            Task::none()
                        }
                    }
                    Page::Billable => {
                        if self.billable.entries.is_empty() && self.client.is_some() {
                            self.loading = true;
                            self.billable_gen += 1;
                            self.load_billable_task()
                        } else {
                            Task::none()
                        }
                    }
                    Page::ProjectTracking => {
                        let year = self.project_tracking.year;
                        if self.project_tracking.entries.is_empty()
                            && self.client.is_some()
                            && !self.project_tracking.budgets.budgets_for(year).is_empty()
                        {
                            self.loading = true;
                            self.project_tracking_gen += 1;
                            self.load_project_tracking_task()
                        } else {
                            Task::none()
                        }
                    }
                };
                self.page = page;
                task
            }

            NavMsg::DatePrev => {
                self.current_date -= chrono::Duration::days(1);
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.work_day_edit = WorkDayEditState::default();
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            NavMsg::DateNext => {
                self.current_date += chrono::Duration::days(1);
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.work_day_edit = WorkDayEditState::default();
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            NavMsg::DateToday => {
                self.current_date = Local::now().naive_local().date();
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.work_day_edit = WorkDayEditState::default();
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            NavMsg::DatePickerToggle => {
                self.date_picker.open = !self.date_picker.open;
                self.date_picker.month = self.current_date;
                Task::none()
            }

            NavMsg::DatePickerMonthPrev => {
                let d = self.date_picker.month;
                self.date_picker.month = if d.month() == 1 {
                    NaiveDate::from_ymd_opt(d.year() - 1, 12, 1).expect("valid backward date")
                } else {
                    NaiveDate::from_ymd_opt(d.year(), d.month() - 1, 1).expect("valid backward date")
                };
                Task::none()
            }

            NavMsg::DatePickerMonthNext => {
                let d = self.date_picker.month;
                self.date_picker.month = if d.month() == 12 {
                    NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).expect("valid forward date")
                } else {
                    NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).expect("valid forward date")
                };
                Task::none()
            }

            NavMsg::DatePickerSelect(date) => {
                self.date_picker.open = false;
                self.current_date = date;
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.work_day_edit = WorkDayEditState::default();
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }
        }
    }
}
