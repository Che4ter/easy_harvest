use super::*;
use super::tasks::format_harvest_error;

// ── Settings sub-state ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SettingsFormState {
    pub token_input: String,
    pub account_input: String,
    pub connecting: bool,
    pub error: Option<String>,
    pub weekly_hours_input: String,
    pub percentage_input: String,
    pub holidays_input: String,
    pub first_work_day_input: String,
    pub profile_saved: bool,
    pub profile_error: Option<String>,
    pub carryover_year_input: String,
    pub carryover_holiday_input: String,
    pub carryover_overtime_input: String,
    pub carryover_error: Option<String>,
    pub holiday_view_year: i32,
    pub cached_holidays: Vec<PublicHoliday>,
    pub holiday_task_query: String,
    pub data_dir_input: String,
    pub data_dir_saved: bool,
    /// Cached deduped task list for holiday_tasks_section: (task_id, task_name, context).
    pub cached_task_list: Vec<(i64, String, String)>,
}

impl SettingsFormState {
    pub fn new(year: i32) -> Self {
        Self {
            holiday_view_year: year,
            cached_holidays: swiss_public_holidays(year),
            ..Default::default()
        }
    }

    pub fn validate_profile(&self) -> Result<ValidatedProfile, String> {
        let weekly_hours: f64 = self.weekly_hours_input.replace(',', ".").parse()
            .ok()
            .filter(|&v: &f64| v > 0.0 && v <= 168.0)
            .ok_or_else(|| "Invalid weekly hours (must be 1\u{2013}168)".to_string())?;
        let percentage: f64 = self.percentage_input.replace(',', ".").parse::<f64>()
            .ok()
            .filter(|&v| v > 0.0 && v <= 100.0)
            .map(|v| v / 100.0)
            .ok_or_else(|| "Invalid percentage (1\u{2013}100)".to_string())?;
        let holidays: u32 = self.holidays_input.parse::<u32>()
            .ok()
            .filter(|&v| v <= 365)
            .ok_or_else(|| "Invalid holiday days".to_string())?;
        let raw = self.first_work_day_input.trim();
        let first_work_day = if raw.is_empty() {
            None
        } else {
            Some(NaiveDate::parse_from_str(raw, "%d.%m.%Y")
                .map_err(|_| "Invalid first work day \u{2014} use DD.MM.YYYY".to_string())?)
        };
        Ok(ValidatedProfile { weekly_hours, percentage, holidays, first_work_day })
    }

    pub fn validate_carryover(&self) -> Result<ValidatedCarryover, String> {
        let year: i32 = self.carryover_year_input.trim().parse()
            .ok()
            .filter(|y: &i32| (2000..=2100).contains(y))
            .ok_or_else(|| "Invalid year (2000\u{2013}2100)".to_string())?;
        let holiday_hours: f64 = self.carryover_holiday_input.replace(',', ".").parse()
            .map_err(|_| "Invalid vacation hours".to_string())?;
        let overtime_hours: f64 = self.carryover_overtime_input.replace(',', ".").parse()
            .map_err(|_| "Invalid overtime hours".to_string())?;
        Ok(ValidatedCarryover { year, holiday_hours, overtime_hours })
    }
}

pub struct ValidatedProfile {
    pub weekly_hours: f64,
    pub percentage: f64,
    pub holidays: u32,
    pub first_work_day: Option<NaiveDate>,
}

pub struct ValidatedCarryover {
    pub year: i32,
    pub holiday_hours: f64,
    pub overtime_hours: f64,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateFormState {
    pub open: bool,
    pub label: String,
    pub project_query: String,
    pub project_idx: Option<usize>,
    pub hours: String,
    pub notes: String,
    pub error: Option<String>,
}

// ── Settings messages ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    Disconnect,
    WizardNext,
    WizardBack,
    WizardUseDefault,
    TokenChanged(String),
    AccountIdChanged(String),
    Save,
    Connected(Result<String, String>),
    WeeklyHoursChanged(String),
    PercentageChanged(String),
    HolidaysChanged(String),
    FirstWorkDayChanged(String),
    SaveProfile,
    CarryoverYearChanged(String),
    CarryoverHolidayChanged(String),
    CarryoverOvertimeChanged(String),
    CarryoverSave,
    CarryoverDelete(i32),
    HolidayTaskToggle(i64),
    HolidayTaskQueryChanged(String),
    DataDirChanged(String),
    PickDataDir,
    DataDirPicked(Option<std::path::PathBuf>),
    SaveDataDir,
    AutostartToggle,
    HolidayViewYearPrev,
    HolidayViewYearNext,
    TemplateAddOpen,
    TemplateAddCancel,
    TemplateAddLabelChanged(String),
    TemplateAddProjectQueryChanged(String),
    TemplateAddProjectSelected(usize),
    TemplateAddHoursChanged(String),
    TemplateAddNotesChanged(String),
    TemplateAddSave,
    TemplateDelete(usize),
}

impl EasyHarvest {
    /// Save settings and surface any error via the error banner.
    fn save_settings_or_warn(&mut self) {
        if let Err(e) = self.settings.save() {
            self.error_banner = Some(format!("Failed to save settings: {e}"));
        }
    }

    pub(super) fn update_settings(&mut self, msg: SettingsMsg) -> Task<Message> {
        match msg {
            SettingsMsg::Disconnect => {
                // Clear token from keyring and file.
                if let Ok(entry) = keyring::Entry::new("easy_harvest", "harvest_api_token") {
                    let _ = entry.delete_credential();
                }
                let _ = std::fs::remove_file(Settings::token_file_path(&self.settings.data_dir));
                self.settings.account_id = String::new();
                self.save_settings_or_warn();
                self.client = None;
                self.harvest_user_id = None;
                self.assignments.clear();
                self.assignments.shrink_to_fit();
                self.entries.clear();
                self.entries.shrink_to_fit();
                let cur_year = chrono::Local::now().naive_local().year();
                self.vacation = VacationPageState::new(cur_year);
                self.billable = BillablePageState::new(cur_year);
                self.project_tracking = ProjectTrackingPageState::new(&self.settings.data_dir, cur_year);
                self.template_form = TemplateFormState::default();
                self.year_balance = None;
                self.holiday_stats = None;
                self.overtime_adj_form = None;
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
                self.project_tracking_gen = 0;
                // Determine wizard step: skip data-folder step if bootstrap exists.
                self.wizard_step =
                    if BootstrapConfig::config_path().exists() { 1 } else { 0 };
                self.page = Page::Settings;
                Task::none()
            }

            SettingsMsg::WizardNext => {
                // Save the chosen data folder and advance to credentials step.
                let new_dir = std::path::PathBuf::from(self.settings_form.data_dir_input.trim());
                if let Err(e) = (BootstrapConfig { data_dir: new_dir.clone() }).save() {
                    self.error_banner = Some(format!("Failed to save bootstrap config: {e}"));
                }
                if new_dir != self.settings.data_dir {
                    self.settings.data_dir = new_dir;
                    self.save_settings_or_warn();
                }
                self.wizard_step = 1;
                Task::none()
            }

            SettingsMsg::WizardBack => {
                self.wizard_step = 0;
                self.settings_form.data_dir_input = self.settings.data_dir.display().to_string();
                Task::none()
            }

            SettingsMsg::WizardUseDefault => {
                self.settings_form.data_dir_input =
                    crate::state::bootstrap::default_data_dir().display().to_string();
                Task::done(Message::Settings(SettingsMsg::WizardNext))
            }

            SettingsMsg::TokenChanged(t) => {
                self.settings_form.token_input = t;
                Task::none()
            }

            SettingsMsg::AccountIdChanged(a) => {
                self.settings_form.account_input = a;
                Task::none()
            }

            SettingsMsg::Save => {
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
                    |result| Message::Settings(SettingsMsg::Connected(result)),
                )
            }

            SettingsMsg::Connected(result) => {
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
                        let client = match HarvestClient::new(token, account_id) {
                            Ok(c) => c,
                            Err(e) => {
                                self.settings_form.error =
                                    Some(format!("Failed to create HTTP client: {e}"));
                                return Task::none();
                            }
                        };
                        self.client = Some(client);
                        self.loading = true;
                        // Load the user ID first; once resolved, CurrentUserLoaded
                        // will dispatch load_entries_task + load_assignments_task
                        // so every request is filtered to the current user.
                        self.harvest_user_id = None;
                        let task = self.load_current_user_task();
                        self.page = Page::Day;
                        task
                    }
                    Err(e) => {
                        self.settings_form.error = Some(e);
                        Task::none()
                    }
                }
            }

            SettingsMsg::WeeklyHoursChanged(v) => {
                self.settings_form.weekly_hours_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            SettingsMsg::PercentageChanged(v) => {
                self.settings_form.percentage_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            SettingsMsg::HolidaysChanged(v) => {
                self.settings_form.holidays_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            SettingsMsg::FirstWorkDayChanged(v) => {
                self.settings_form.first_work_day_input = v;
                self.settings_form.profile_saved = false;
                Task::none()
            }

            SettingsMsg::CarryoverYearChanged(v) => {
                self.settings_form.carryover_year_input = v;
                Task::none()
            }

            SettingsMsg::CarryoverHolidayChanged(v) => {
                self.settings_form.carryover_holiday_input = v;
                Task::none()
            }

            SettingsMsg::CarryoverOvertimeChanged(v) => {
                self.settings_form.carryover_overtime_input = v;
                Task::none()
            }

            SettingsMsg::CarryoverSave => {
                let validated = match self.settings_form.validate_carryover() {
                    Ok(v) => v,
                    Err(e) => {
                        self.settings_form.carryover_error = Some(e);
                        return Task::none();
                    }
                };
                let epd = self.settings.expected_hours_per_day();
                self.settings.carryover.insert(validated.year, crate::state::settings::YearCarryover {
                    holiday_days: if epd > 0.0 { validated.holiday_hours / epd } else { 0.0 },
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

            SettingsMsg::CarryoverDelete(year) => {
                self.settings.carryover.remove(&year);
                self.save_settings_or_warn();
                Task::none()
            }

            SettingsMsg::SaveProfile => {
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

            SettingsMsg::HolidayTaskToggle(task_id) => {
                if self.settings.holiday_task_ids.contains(&task_id) {
                    self.settings.holiday_task_ids.retain(|&id| id != task_id);
                } else {
                    self.settings.holiday_task_ids.push(task_id);
                    self.settings_form.holiday_task_query.clear();
                }
                self.save_settings_or_warn();
                Task::none()
            }

            SettingsMsg::HolidayTaskQueryChanged(v) => {
                self.settings_form.holiday_task_query = v;
                Task::none()
            }

            SettingsMsg::HolidayViewYearPrev => {
                self.settings_form.holiday_view_year -= 1;
                self.settings_form.cached_holidays = swiss_public_holidays(self.settings_form.holiday_view_year);
                Task::none()
            }

            SettingsMsg::HolidayViewYearNext => {
                self.settings_form.holiday_view_year += 1;
                self.settings_form.cached_holidays = swiss_public_holidays(self.settings_form.holiday_view_year);
                Task::none()
            }

            SettingsMsg::TemplateAddOpen => {
                self.template_form.open = true;
                self.template_form.label = String::new();
                self.template_form.project_query = String::new();
                self.template_form.project_idx = None;
                self.template_form.hours = String::new();
                self.template_form.notes = String::new();
                self.template_form.error = None;
                Task::none()
            }

            SettingsMsg::TemplateAddCancel => {
                self.template_form.open = false;
                Task::none()
            }

            SettingsMsg::TemplateAddLabelChanged(v) => {
                self.template_form.label = v;
                Task::none()
            }

            SettingsMsg::TemplateAddProjectQueryChanged(v) => {
                self.template_form.project_query = v;
                self.template_form.project_idx = None;
                Task::none()
            }

            SettingsMsg::TemplateAddProjectSelected(idx) => {
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

            SettingsMsg::TemplateAddHoursChanged(v) => {
                self.template_form.hours = v;
                Task::none()
            }

            SettingsMsg::TemplateAddNotesChanged(v) => {
                self.template_form.notes = v;
                Task::none()
            }

            SettingsMsg::TemplateAddSave => {
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

            SettingsMsg::TemplateDelete(idx) => {
                if idx < self.templates.entries.len() {
                    self.templates.entries.remove(idx);
                    if let Err(e) = self.templates.save(&self.settings.data_dir) {
                        self.error_banner = Some(format!("Failed to save templates: {e}"));
                    }
                }
                Task::none()
            }

            SettingsMsg::DataDirChanged(v) => {
                self.settings_form.data_dir_input = v;
                self.settings_form.data_dir_saved = false;
                Task::none()
            }

            SettingsMsg::PickDataDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose data folder")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                |result| Message::Settings(SettingsMsg::DataDirPicked(result)),
            ),

            SettingsMsg::DataDirPicked(maybe_path) => {
                if let Some(path) = maybe_path {
                    self.settings_form.data_dir_input = path.display().to_string();
                }
                Task::none()
            }

            SettingsMsg::SaveDataDir => {
                let new_dir = std::path::PathBuf::from(self.settings_form.data_dir_input.trim());
                if new_dir == self.settings.data_dir {
                    self.settings_form.data_dir_saved = true;
                    return Task::none();
                }
                // Write bootstrap pointer
                if let Err(e) = (BootstrapConfig { data_dir: new_dir.clone() }).save() {
                    self.error_banner = Some(format!("Failed to save bootstrap config: {e}"));
                }
                // Copy token to new location.
                if let Some(token) = Settings::load_token(&self.settings.data_dir) {
                    if let Err(e) = Settings::save_token(&token, &new_dir) {
                        self.error_banner = Some(format!("Failed to copy token: {e}"));
                    }
                }
                // Copy all other data files so the folder change is non-destructive.
                let old_dir = self.settings.data_dir.clone();
                let migrate_errors = migrate_data_files(&old_dir, &new_dir);
                if !migrate_errors.is_empty() {
                    self.error_banner = Some(format!(
                        "Data folder migration: some files could not be copied — {}",
                        migrate_errors.join("; ")
                    ));
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
                self.vacation = VacationPageState::new(self.current_date.year());
                self.billable = BillablePageState::new(self.current_date.year());
                self.project_tracking = ProjectTrackingPageState::new(&new_dir, self.current_date.year());
                self.year_balance = None;
                self.holiday_stats = None;
                self.overtime_adjustments = OvertimeAdjustmentStore::load(&new_dir);
                self.overtime_adj_form = None;
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

            SettingsMsg::AutostartToggle => {
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
        }
    }
}

// ── Data-folder migration helper ──────────────────────────────────────────────

/// Copy all known data files from `old_dir` to `new_dir` so that a folder
/// change is non-destructive.  Files that do not exist in the old location are
/// skipped silently.  Existing files in `new_dir` are overwritten so the old
/// data always wins.
///
/// Returns a list of human-readable error strings for any file that could not
/// be copied so the caller can surface them to the user.
fn migrate_data_files(
    old_dir: &std::path::Path,
    new_dir: &std::path::Path,
) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    let files = [
        "favorites.json",
        "templates.json",
        "overtime_adjustments.json",
        "project_budgets.json",
    ];
    for name in &files {
        let src = old_dir.join(name);
        if src.exists() {
            if let Err(e) = std::fs::copy(&src, new_dir.join(name)) {
                errors.push(format!("{name}: {e}"));
            }
        }
    }

    // Copy the work_days/ directory recursively.
    let src_wd = old_dir.join("work_days");
    if src_wd.is_dir() {
        let dst_wd = new_dir.join("work_days");
        if let Err(e) = std::fs::create_dir_all(&dst_wd) {
            errors.push(format!("work_days/ (mkdir): {e}"));
        } else if let Ok(entries) = std::fs::read_dir(&src_wd) {
            for entry in entries.flatten() {
                let src_file = entry.path();
                if src_file.is_file() {
                    if let Some(name) = src_file.file_name() {
                        if let Err(e) = std::fs::copy(&src_file, dst_wd.join(name)) {
                            errors.push(format!("work_days/{}: {e}", name.to_string_lossy()));
                        }
                    }
                }
            }
        }
    }

    // Copy the cache/ directory — best-effort, not reported as error since it
    // is just a download cache that will be refreshed on next API call.
    let src_cache = old_dir.join("cache");
    if src_cache.is_dir() {
        let dst_cache = new_dir.join("cache");
        if std::fs::create_dir_all(&dst_cache).is_ok() {
            if let Ok(entries) = std::fs::read_dir(&src_cache) {
                for entry in entries.flatten() {
                    let src_file = entry.path();
                    if src_file.is_file() {
                        if let Some(name) = src_file.file_name() {
                            let _ = std::fs::copy(&src_file, dst_cache.join(name));
                        }
                    }
                }
            }
        }
    }

    errors
}

