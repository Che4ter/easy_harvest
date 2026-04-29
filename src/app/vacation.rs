use super::*;
use super::tasks::build_vacation_entries;

// ── Vacation form ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VacationForm {
    pub from_input: String,
    pub to_input: String,
    /// true = full day, false = half day
    pub full_day: bool,
    /// Which holiday task to book against (relevant when multiple are configured).
    pub selected_task_id: Option<i64>,
    pub error: Option<String>,
    pub submitting: bool,
}

impl VacationForm {
    pub fn new() -> Self {
        Self {
            from_input: String::new(),
            to_input: String::new(),
            full_day: true,
            selected_task_id: None,
            error: None,
            submitting: false,
        }
    }
}

impl Default for VacationForm {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct VacationSummary {
    pub used_days: f64,
    pub booked_days: f64,
    pub days_remaining: f64,
    pub total_days: f64,
    pub carryover_days: f64,
}

#[derive(Debug, Clone)]
pub struct VacationPageState {
    pub entries: Vec<TimeEntry>,
    pub year: i32,
    pub form: Option<VacationForm>,
    pub summary: Option<VacationSummary>,
}

impl VacationPageState {
    pub fn new(year: i32) -> Self {
        Self {
            entries: Vec::new(),
            year,
            form: None,
            summary: None,
        }
    }
}

impl Default for VacationPageState {
    fn default() -> Self {
        Self::new(0)
    }
}

// ── Vacation messages ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum VacationMsg {
    Refresh,
    YearPrev,
    YearNext,
    EntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    ShowForm,
    HideForm,
    FromChanged(String),
    ToChanged(String),
    DayTypeFull,
    DayTypeHalf,
    TaskSelected(i64),
    FormSubmit,
    EntriesCreated(Result<Vec<TimeEntry>, String>),
    DeleteEntry(i64),
    EntryDeleted(Result<i64, String>),
}

impl EasyHarvest {
    pub(super) fn update_vacation(&mut self, msg: VacationMsg) -> Task<Message> {
        match msg {
            VacationMsg::YearPrev => {
                self.vacation.year -= 1;
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                self.vacation.form = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.vacation_gen += 1;
                    self.load_vacation_task()
                } else {
                    Task::none()
                }
            }

            VacationMsg::YearNext => {
                self.vacation.year += 1;
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                self.vacation.form = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.vacation_gen += 1;
                    self.load_vacation_task()
                } else {
                    Task::none()
                }
            }

            VacationMsg::Refresh => {
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.vacation_gen += 1;
                    self.load_vacation_task()
                } else {
                    Task::none()
                }
            }

            VacationMsg::EntriesLoaded(r#gen, result) => {
                if r#gen != self.vacation_gen { return Task::none(); }
                self.loading = false;
                match result {
                    Ok(entries) => {
                        self.vacation.entries = entries;
                        self.recompute_vacation_summary();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            VacationMsg::ShowForm => {
                let mut form = VacationForm::new();
                form.selected_task_id = self.settings.holiday_task_ids.first().copied();
                self.vacation.form = Some(form);
                Task::none()
            }

            VacationMsg::HideForm => {
                self.vacation.form = None;
                Task::none()
            }

            VacationMsg::FromChanged(v) => {
                if let Some(f) = &mut self.vacation.form {
                    f.from_input = v;
                    f.error = None;
                }
                Task::none()
            }

            VacationMsg::ToChanged(v) => {
                if let Some(f) = &mut self.vacation.form {
                    f.to_input = v;
                    f.error = None;
                }
                Task::none()
            }

            VacationMsg::DayTypeFull => {
                if let Some(f) = &mut self.vacation.form { f.full_day = true; }
                Task::none()
            }

            VacationMsg::DayTypeHalf => {
                if let Some(f) = &mut self.vacation.form { f.full_day = false; }
                Task::none()
            }

            VacationMsg::TaskSelected(task_id) => {
                if let Some(f) = &mut self.vacation.form {
                    f.selected_task_id = Some(task_id);
                    f.error = None;
                }
                Task::none()
            }

            VacationMsg::FormSubmit => {
                let Some(form) = &mut self.vacation.form else { return Task::none(); };
                let parse = |s: &str| {
                    NaiveDate::parse_from_str(s.trim(), "%d.%m.%Y")
                        .or_else(|_| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d"))
                        .ok()
                };
                let from = parse(&form.from_input.clone());
                let to_str = if form.to_input.trim().is_empty() {
                    form.from_input.clone()
                } else {
                    form.to_input.clone()
                };
                let to = parse(&to_str);
                match (from, to) {
                    (Some(from), Some(to)) => {
                        let full_day = form.full_day;
                        let hours_full = self.settings.expected_hours_per_day();
                        let hours = if full_day { hours_full } else { hours_full / 2.0 };
                        // find project_id + task_id from assignments
                        let Some(task_id) = form.selected_task_id else {
                            form.error = Some("Please select a holiday task.".into());
                            return Task::none();
                        };
                        let project_id = self.assignments.iter().find_map(|a| {
                            if a.task_assignments.iter().any(|t| t.task.id == task_id) {
                                Some(a.project.id)
                            } else {
                                None
                            }
                        });
                        let Some(project_id) = project_id else {
                            form.error = Some("Could not find project for holiday task. Try syncing assignments.".into());
                            return Task::none();
                        };
                        match build_vacation_entries(from, to, self.vacation.year, hours, project_id, task_id) {
                            Ok(entries_to_create) => {
                                form.submitting = true;
                                form.error = None;
                                self.submit_vacation_task(entries_to_create)
                            }
                            Err(e) => {
                                form.error = Some(e);
                                Task::none()
                            }
                        }
                    }
                    _ => {
                        if let Some(f) = &mut self.vacation.form {
                            f.error = Some("Enter valid dates in DD.MM.YYYY format. From must be \u{2264} To.".into());
                        }
                        Task::none()
                    }
                }
            }

            VacationMsg::EntriesCreated(result) => {
                if let Some(f) = &mut self.vacation.form {
                    f.submitting = false;
                }
                match result {
                    Ok(new_entries) => {
                        self.vacation.form = None;
                        // Only apply entries that match the currently displayed year;
                        // discard stale results that arrived after a year navigation.
                        let current_year = self.vacation.year;
                        let year_prefix = format!("{}-", current_year);
                        let matching: Vec<_> = new_entries
                            .into_iter()
                            .filter(|e| e.spent_date.starts_with(&year_prefix))
                            .collect();
                        if !matching.is_empty() {
                            self.vacation.entries.extend(matching);
                            self.vacation.entries.sort_by(|a, b| a.spent_date.cmp(&b.spent_date));
                            self.recompute_vacation_summary();
                        }
                    }
                    Err(e) => {
                        if let Some(f) = &mut self.vacation.form {
                            f.error = Some(e);
                        }
                    }
                }
                Task::none()
            }

            VacationMsg::DeleteEntry(id) => {
                let Some(client) = self.client.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        client.delete_time_entry(id).await
                            .map(|_| id)
                            .map_err(|e| e.to_string())
                    },
                    |result| Message::Vacation(VacationMsg::EntryDeleted(result)),
                )
            }

            VacationMsg::EntryDeleted(result) => {
                match result {
                    Ok(id) => {
                        self.vacation.entries.retain(|e| e.id != id);
                        self.recompute_vacation_summary();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }
        }
    }
}
