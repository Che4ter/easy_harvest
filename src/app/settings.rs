use super::*;
use super::tasks::format_harvest_error;

// ── Settings ─────────────────────────────────────────────────────────────────

impl EasyHarvest {
    /// Save settings and surface any error via the error banner.
    fn save_settings_or_warn(&mut self) {
        if let Err(e) = self.settings.save() {
            self.error_banner = Some(format!("Failed to save settings: {e}"));
        }
    }

    pub(super) fn update_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Disconnect => {
                // Clear token from keyring and file.
                if let Ok(entry) = keyring::Entry::new("easy_harvest", "harvest_api_token") {
                    let _ = entry.delete_credential();
                }
                let _ = std::fs::remove_file(Settings::token_file_path(&self.settings.data_dir));
                self.settings.account_id = String::new();
                self.save_settings_or_warn();
                self.client = None;
                self.assignments.clear();
                self.assignments.shrink_to_fit();
                self.entries.clear();
                self.entries.shrink_to_fit();
                self.vacation = VacationPageState::default();
                self.billable = BillablePageState::default();
                self.template_form = TemplateFormState::default();
                self.year_balance = None;
                self.holiday_stats = None;
                self.entry_form = None;
                self.pending_delete = None;
                self.templates = Templates::default();
                self.cached_project_options = Vec::new();
                self.error_banner = None;
                self.loading = false;
                self.entries_gen = 0;
                self.vacation_gen = 0;
                self.billable_gen = 0;
                self.stats_gen = 0;
                // Determine wizard step: skip data-folder step if bootstrap exists.
                self.wizard_step =
                    if BootstrapConfig::config_path().exists() { 1 } else { 0 };
                self.page = Page::Settings;
                Task::none()
            }

            Message::WizardNext => {
                // Save the chosen data folder and advance to credentials step.
                let new_dir = std::path::PathBuf::from(self.settings_form.data_dir_input.trim());
                let _ = BootstrapConfig { data_dir: new_dir.clone() }.save();
                if new_dir != self.settings.data_dir {
                    self.settings.data_dir = new_dir;
                    self.save_settings_or_warn();
                }
                self.wizard_step = 1;
                Task::none()
            }

            Message::WizardBack => {
                self.wizard_step = 0;
                self.settings_form.data_dir_input = self.settings.data_dir.display().to_string();
                Task::none()
            }

            Message::SettingsTokenChanged(t) => {
                self.settings_form.token_input = t;
                Task::none()
            }

            Message::SettingsAccountIdChanged(a) => {
                self.settings_form.account_input = a;
                Task::none()
            }

            Message::SettingsSave => {
                let token = self.settings_form.token_input.trim().to_string();
                let account_id = self.settings_form.account_input.trim().to_string();
                if token.is_empty() || account_id.is_empty() {
                    self.settings_form.error =
                        Some("Token and Account ID are required".into());
                    return Task::none();
                }
                self.settings_form.connecting = true;
                self.settings_form.error = None;
                let client = match HarvestClient::new(token.clone(), account_id.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        self.settings_form.connecting = false;
                        self.settings_form.error = Some(format!("Failed to create HTTP client: {e}"));
                        return Task::none();
                    }
                };
                Task::perform(
                    async move {
                        client
                            .get_current_user()
                            .await
                            .map(|u| format!("{} {}", u.first_name, u.last_name))
                            .map_err(format_harvest_error)
                    },
                    Message::SettingsConnected,
                )
            }

            Message::SettingsConnected(result) => {
                self.settings_form.connecting = false;
                match result {
                    Ok(_) => {
                        let token = self.settings_form.token_input.trim().to_string();
                        let account_id =
                            self.settings_form.account_input.trim().to_string();
                        if let Err(e) = Settings::save_token(&token, &self.settings.data_dir) {
                            self.error_banner = Some(format!("Failed to save token: {e}"));
                        }
                        self.settings.account_id = account_id.clone();
                        self.save_settings_or_warn();
                        self.client = HarvestClient::new(token, account_id).ok();
                        self.loading = true;
                        self.entries_gen += 1;
                        let task = Task::batch([
                            self.load_entries_task(),
                            self.load_assignments_task(),
                        ]);
                        self.page = Page::Day;
                        task
                    }
                    Err(e) => {
                        self.settings_form.error = Some(e);
                        Task::none()
                    }
                }
            }

            Message::SettingsWeeklyHoursChanged(v) => {
                self.settings_form.weekly_hours_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            Message::SettingsPercentageChanged(v) => {
                self.settings_form.percentage_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            Message::SettingsHolidaysChanged(v) => {
                self.settings_form.holidays_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            Message::SettingsFirstWorkDayChanged(v) => {
                self.settings_form.first_work_day_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            Message::SettingsCarryoverYearChanged(v) => {
                self.settings_form.carryover_year_input = v;
                Task::none()
            }

            Message::SettingsCarryoverHolidayChanged(v) => {
                self.settings_form.carryover_holiday_input = v;
                Task::none()
            }

            Message::SettingsCarryoverOvertimeChanged(v) => {
                self.settings_form.carryover_overtime_input = v;
                Task::none()
            }

            Message::SettingsCarryoverSave => {
                let validated = match self.settings_form.validate_carryover() {
                    Ok(v) => v,
                    Err(e) => {
                        self.settings_form.carryover_error = Some(e);
                        return Task::none();
                    }
                };
                self.settings.carryover.insert(validated.year, crate::state::settings::YearCarryover {
                    holiday_days: validated.holiday_days,
                    overtime_hours: validated.overtime_hours,
                });
                match self.settings.save() {
                    Ok(()) => {
                        self.settings_form.carryover_year_input.clear();
                        self.settings_form.carryover_holiday_input.clear();
                        self.settings_form.carryover_overtime_input.clear();
                        self.settings_form.carryover_error = None;
                    }
                    Err(e) => self.settings_form.carryover_error = Some(format!("Save failed: {e}")),
                }
                Task::none()
            }

            Message::SettingsCarryoverDelete(year) => {
                self.settings.carryover.remove(&year);
                self.save_settings_or_warn();
                Task::none()
            }

            Message::SettingsSaveProfile => {
                let profile = match self.settings_form.validate_profile() {
                    Ok(p) => p,
                    Err(e) => {
                        self.settings_form.profile_error = Some(e);
                        return Task::none();
                    }
                };
                self.settings.total_weekly_hours = profile.weekly_hours;
                self.settings.work_percentage = profile.percentage;
                self.settings.total_holiday_days_per_year = profile.holidays;
                self.settings.first_work_day = profile.first_work_day;
                match self.settings.save() {
                    Ok(()) => {
                        self.settings_form.profile_saved = true;
                        self.settings_form.profile_error = None;
                        self.recompute_expected_hours();
                    }
                    Err(e) => {
                        self.settings_form.profile_error = Some(format!("Save failed: {e}"));
                    }
                }
                Task::none()
            }

            Message::HolidayTaskToggle(task_id) => {
                if self.settings.holiday_task_ids.contains(&task_id) {
                    self.settings.holiday_task_ids.retain(|&id| id != task_id);
                } else {
                    self.settings.holiday_task_ids.push(task_id);
                    self.settings_form.holiday_task_query.clear();
                }
                self.save_settings_or_warn();
                Task::none()
            }

            Message::HolidayTaskQueryChanged(v) => {
                self.settings_form.holiday_task_query = v;
                Task::none()
            }

            Message::HolidayViewYearPrev => {
                self.settings_form.holiday_view_year -= 1;
                self.settings_form.cached_holidays = swiss_public_holidays(self.settings_form.holiday_view_year);
                Task::none()
            }

            Message::HolidayViewYearNext => {
                self.settings_form.holiday_view_year += 1;
                self.settings_form.cached_holidays = swiss_public_holidays(self.settings_form.holiday_view_year);
                Task::none()
            }

            Message::SettingsTemplateAddOpen => {
                self.template_form.open = true;
                self.template_form.label = String::new();
                self.template_form.project_query = String::new();
                self.template_form.project_idx = None;
                self.template_form.hours = String::new();
                self.template_form.notes = String::new();
                self.template_form.error = None;
                Task::none()
            }

            Message::SettingsTemplateAddCancel => {
                self.template_form.open = false;
                Task::none()
            }

            Message::SettingsTemplateAddLabelChanged(v) => {
                self.template_form.label = v;
                Task::none()
            }

            Message::SettingsTemplateAddProjectQueryChanged(v) => {
                self.template_form.project_query = v;
                self.template_form.project_idx = None;
                Task::none()
            }

            Message::SettingsTemplateAddProjectSelected(idx) => {
                let opts = self.cached_project_options.clone();
                if let Some(opt) = opts.get(idx) {
                    self.template_form.project_query = format!(
                        "{} \u{203a} {} \u{2014} {}",
                        opt.client_name, opt.project_name, opt.task_name
                    );
                    self.template_form.project_idx = Some(idx);
                }
                Task::none()
            }

            Message::SettingsTemplateAddHoursChanged(v) => {
                self.template_form.hours = v;
                Task::none()
            }

            Message::SettingsTemplateAddNotesChanged(v) => {
                self.template_form.notes = v;
                Task::none()
            }

            Message::SettingsTemplateAddSave => {
                let label = self.template_form.label.trim().to_owned();
                if label.is_empty() {
                    self.template_form.error = Some("Please enter a name.".into());
                    return Task::none();
                }
                let Some(idx) = self.template_form.project_idx else {
                    self.template_form.error = Some("Please select a project and task.".into());
                    return Task::none();
                };
                let opts = self.cached_project_options.clone();
                let Some(opt) = opts.get(idx) else {
                    self.template_form.error = Some("Project not found.".into());
                    return Task::none();
                };
                let tpl = crate::state::templates::EntryTemplate {
                    label,
                    project_id: opt.project_id,
                    task_id: opt.task_id,
                    hours: self.template_form.hours.trim().to_owned(),
                    notes: self.template_form.notes.trim().to_owned(),
                };
                self.templates.entries.push(tpl);
                if let Err(e) = self.templates.save(&self.settings.data_dir) {
                    self.error_banner = Some(format!("Failed to save template: {e}"));
                }
                self.template_form.open = false;
                Task::none()
            }

            Message::SettingsTemplateDelete(idx) => {
                if idx < self.templates.entries.len() {
                    self.templates.entries.remove(idx);
                    if let Err(e) = self.templates.save(&self.settings.data_dir) {
                        self.error_banner = Some(format!("Failed to save templates: {e}"));
                    }
                }
                Task::none()
            }

            Message::SettingsDataDirChanged(v) => {
                self.settings_form.data_dir_input = v;
                self.settings_form.data_dir_saved = false;
                Task::none()
            }

            Message::SettingsPickDataDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose data folder")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::SettingsDataDirPicked,
            ),

            Message::SettingsDataDirPicked(maybe_path) => {
                if let Some(path) = maybe_path {
                    self.settings_form.data_dir_input = path.display().to_string();
                }
                Task::none()
            }

            Message::SettingsSaveDataDir => {
                let new_dir = std::path::PathBuf::from(self.settings_form.data_dir_input.trim());
                if new_dir == self.settings.data_dir {
                    self.settings_form.data_dir_saved = true;
                    return Task::none();
                }
                // Write bootstrap pointer
                if let Err(e) = (BootstrapConfig { data_dir: new_dir.clone() }).save() {
                    self.error_banner = Some(format!("Failed to save bootstrap config: {e}"));
                }
                // Copy token file to new location
                if let Some(token) = Settings::load_token(&self.settings.data_dir) {
                    if let Err(e) = Settings::save_token(&token, &new_dir) {
                        self.error_banner = Some(format!("Failed to copy token: {e}"));
                    }
                }
                // Move to new data dir
                self.settings.data_dir = new_dir.clone();
                self.save_settings_or_warn();
                self.work_day_store = crate::state::persistence::WorkDayStore::load(
                    &new_dir,
                    self.current_date.year(),
                    self.current_date.month(),
                );
                self.favorites = Favorites::load(&new_dir);
                self.templates = Templates::load(&new_dir);
                self.recompute_project_options();
                // Clear stale API-fetched data
                self.assignments.clear();
                self.entries.clear();
                self.vacation = VacationPageState::default();
                self.billable = BillablePageState::default();
                self.year_balance = None;
                self.holiday_stats = None;
                self.settings_form.data_dir_saved = true;
                if self.client.is_some() {
                    self.loading = true;
                    self.entries_gen += 1;
                    Task::batch([
                        self.load_entries_task(),
                        self.load_assignments_task(),
                    ])
                } else {
                    Task::none()
                }
            }

            Message::SettingsAutostartToggle => {
                let enabled = !self.settings.autostart;
                let result = if enabled {
                    crate::autostart::enable()
                } else {
                    crate::autostart::disable()
                };
                match result {
                    Ok(()) => {
                        self.settings.autostart = enabled;
                        self.save_settings_or_warn();
                    }
                    Err(e) => {
                        self.error_banner = Some(format!("Autostart: {e}"));
                    }
                }
                Task::none()
            }

            _ => unreachable!(),
        }
    }
}
