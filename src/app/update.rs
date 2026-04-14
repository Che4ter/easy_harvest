use super::*;

// ── Update ────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ── Navigation / Date ──
            Message::PageChanged(_)
            | Message::DatePrev
            | Message::DateNext
            | Message::DateToday
            | Message::DatePickerToggle
            | Message::DatePickerMonthPrev
            | Message::DatePickerMonthNext
            | Message::DatePickerSelect(_) => self.update_navigation(message),

            // ── Entries / Timer ──
            Message::EntriesLoaded(..)
            | Message::AssignmentsLoaded(_)
            | Message::SyncAssignments
            | Message::StatsLoaded(..)
            | Message::ShowAddForm
            | Message::EditEntry(_)
            | Message::CancelForm
            | Message::FormProjectQueryChanged(_)
            | Message::FormProjectSelected(_)
            | Message::FormHoursChanged(_)
            | Message::FormNotesChanged(_)
            | Message::FormFocusHours
            | Message::FormFocusNotes
            | Message::FormSubmit
            | Message::EntryCreated(_)
            | Message::EntryUpdated(_)
            | Message::DeleteRequest(_)
            | Message::DeleteCancel
            | Message::DeleteEntry(_)
            | Message::EntryDeleted(_)
            | Message::TimerStart(_)
            | Message::TimerStop(_)
            | Message::TimerStarted(_)
            | Message::TimerStopped(_)
            | Message::TemplateApply(_) => self.update_entries(message),

            // ── Work Day ──
            Message::StartDay
            | Message::StartBreak
            | Message::EndBreak
            | Message::EndDay
            | Message::ResumeDay
            | Message::WorkDayTick
            | Message::WorkDayEditStart
            | Message::WorkDayEditCancel
            | Message::WorkDayStartInputChanged(_)
            | Message::WorkDayEndInputChanged(_)
            | Message::WorkDayBreakStartChanged(..)
            | Message::WorkDayBreakEndChanged(..)
            | Message::WorkDayBreakDelete(_)
            | Message::WorkDayBreakAdd
            | Message::WorkDayEditSave => self.update_work_day(message),

            // ── Settings ──
            Message::Disconnect
            | Message::WizardNext
            | Message::WizardBack
            | Message::SettingsTokenChanged(_)
            | Message::SettingsAccountIdChanged(_)
            | Message::SettingsSave
            | Message::SettingsConnected(_)
            | Message::SettingsWeeklyHoursChanged(_)
            | Message::SettingsPercentageChanged(_)
            | Message::SettingsHolidaysChanged(_)
            | Message::SettingsFirstWorkDayChanged(_)
            | Message::SettingsCarryoverYearChanged(_)
            | Message::SettingsCarryoverHolidayChanged(_)
            | Message::SettingsCarryoverOvertimeChanged(_)
            | Message::SettingsCarryoverSave
            | Message::SettingsCarryoverDelete(_)
            | Message::SettingsSaveProfile
            | Message::HolidayTaskToggle(_)
            | Message::HolidayTaskQueryChanged(_)
            | Message::HolidayViewYearPrev
            | Message::HolidayViewYearNext
            | Message::SettingsTemplateAddOpen
            | Message::SettingsTemplateAddCancel
            | Message::SettingsTemplateAddLabelChanged(_)
            | Message::SettingsTemplateAddProjectQueryChanged(_)
            | Message::SettingsTemplateAddProjectSelected(_)
            | Message::SettingsTemplateAddHoursChanged(_)
            | Message::SettingsTemplateAddNotesChanged(_)
            | Message::SettingsTemplateAddSave
            | Message::SettingsTemplateDelete(_)
            | Message::SettingsDataDirChanged(_)
            | Message::SettingsPickDataDir
            | Message::SettingsDataDirPicked(_)
            | Message::SettingsSaveDataDir
            | Message::SettingsAutostartToggle => self.update_settings(message),

            // ── Vacation ──
            Message::VacationYearPrev
            | Message::VacationYearNext
            | Message::VacationRefresh
            | Message::VacationEntriesLoaded(..)
            | Message::VacationShowForm
            | Message::VacationHideForm
            | Message::VacationFromChanged(_)
            | Message::VacationToChanged(_)
            | Message::VacationDayTypeFull
            | Message::VacationDayTypeHalf
            | Message::VacationFormSubmit
            | Message::VacationEntriesCreated(_)
            | Message::VacationDeleteEntry(_)
            | Message::VacationEntryDeleted(_) => self.update_vacation(message),

            // ── Billable ──
            Message::BillableYearPrev
            | Message::BillableYearNext
            | Message::BillableRefresh
            | Message::BillableEntriesLoaded(..)
            | Message::BillableMonthSelected(_)
            | Message::BillableMonthClear => self.update_billable(message),

            // ── Stats ──
            Message::StatsRefresh
            | Message::OvertimeYearPrev
            | Message::OvertimeYearNext => self.update_stats(message),

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
                Task::none()
            }

            Message::QuitApp => iced::exit(),

            Message::TrayToggle => {
                if let Some(id) = self.window_id {
                    // Window is open — close it.
                    self.window_id = None;
                    self.window_visible = false;
                    window::close(id)
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

    fn update_navigation(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PageChanged(page) => {
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
                        self.settings_form.percentage_input = format!("{}", (self.settings.work_percentage * 100.0).round() as u32);
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
                };
                self.page = page;
                task
            }

            Message::DatePrev => {
                self.current_date -= chrono::Duration::days(1);
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            Message::DateNext => {
                self.current_date += chrono::Duration::days(1);
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            Message::DateToday => {
                self.current_date = Local::now().naive_local().date();
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            Message::DatePickerToggle => {
                self.date_picker.open = !self.date_picker.open;
                self.date_picker.month = self.current_date;
                Task::none()
            }

            Message::DatePickerMonthPrev => {
                let d = self.date_picker.month;
                self.date_picker.month = if d.month() == 1 {
                    NaiveDate::from_ymd_opt(d.year() - 1, 12, 1).expect("valid backward date")
                } else {
                    NaiveDate::from_ymd_opt(d.year(), d.month() - 1, 1).expect("valid backward date")
                };
                Task::none()
            }

            Message::DatePickerMonthNext => {
                let d = self.date_picker.month;
                self.date_picker.month = if d.month() == 12 {
                    NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).expect("valid forward date")
                } else {
                    NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).expect("valid forward date")
                };
                Task::none()
            }

            Message::DatePickerSelect(date) => {
                self.date_picker.open = false;
                self.current_date = date;
                self.maybe_reload_work_day_store();
                self.recompute_expected_hours();
                self.entries.clear();
                self.entry_form = None;
                self.loading = true;
                self.entries_gen += 1;
                self.load_entries_task()
            }

            _ => unreachable!(),
        }
    }
}
