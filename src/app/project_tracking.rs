use super::*;
use crate::state::project_budgets::{ProjectBudget, ProjectBudgetStore};

// ── Page state ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BudgetSummary {
    pub budget: ProjectBudget,
    pub used_hours: f64,
    pub remaining_hours: f64,
    pub pct_used: f64,
}

#[derive(Debug, Clone, Default)]
pub struct BudgetForm {
    pub name_input: String,
    pub budget_hours_input: String,
    pub project_query: String,
    /// Selected (project_id, project_name, client_name) tuples.
    pub selected_projects: Vec<(i64, String, String)>,
    pub editing_id: Option<u64>,
    pub error: Option<String>,
}

pub struct ValidatedBudget {
    pub name: String,
    pub budget_hours: f64,
    pub project_ids: Vec<i64>,
    pub editing_id: Option<u64>,
}

impl BudgetForm {
    pub fn validate(&self) -> Result<ValidatedBudget, String> {
        let name = self.name_input.trim().to_string();
        if name.is_empty() {
            return Err("Name is required.".into());
        }
        let budget_hours: f64 = self.budget_hours_input.replace(',', ".").parse()
            .ok()
            .filter(|&v: &f64| v > 0.0)
            .ok_or_else(|| "Enter a valid positive number for budget hours.".to_string())?;
        if self.selected_projects.is_empty() {
            return Err("Select at least one project.".into());
        }
        let project_ids = self.selected_projects.iter().map(|(id, _, _)| *id).collect();
        Ok(ValidatedBudget { name, budget_hours, project_ids, editing_id: self.editing_id })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTrackingPageState {
    pub budgets: ProjectBudgetStore,
    pub year: i32,
    pub entries: Vec<TimeEntry>,
    pub summaries: Vec<BudgetSummary>,
    pub form: Option<BudgetForm>,
}

impl ProjectTrackingPageState {
    pub fn new(data_dir: &std::path::Path, year: i32) -> Self {
        Self {
            budgets: ProjectBudgetStore::load(data_dir),
            year,
            ..Default::default()
        }
    }
}

// ── Messages ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ProjectTrackingMsg {
    Refresh,
    YearPrev,
    YearNext,
    EntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    ShowForm,
    HideForm,
    EditBudget(u64),
    DeleteBudget(u64),
    // Form field messages
    NameChanged(String),
    BudgetHoursChanged(String),
    ProjectQueryChanged(String),
    ProjectSelected(usize),
    ProjectRemoved(i64),
    FormSubmit,
}

// ── Update ──────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(super) fn update_project_tracking(&mut self, msg: ProjectTrackingMsg) -> Task<Message> {
        match msg {
            ProjectTrackingMsg::YearPrev => {
                self.project_tracking.year -= 1;
                self.project_tracking.entries.clear();
                self.project_tracking.entries.shrink_to_fit();
                self.project_tracking.summaries.clear();
                self.project_tracking.form = None;
                let year = self.project_tracking.year;
                if self.client.is_some() && !self.project_tracking.budgets.budgets_for(year).is_empty() {
                    self.loading = true;
                    self.project_tracking_gen += 1;
                    self.load_project_tracking_task()
                } else {
                    self.recompute_project_tracking_summaries();
                    Task::none()
                }
            }

            ProjectTrackingMsg::YearNext => {
                self.project_tracking.year += 1;
                self.project_tracking.entries.clear();
                self.project_tracking.entries.shrink_to_fit();
                self.project_tracking.summaries.clear();
                self.project_tracking.form = None;
                let year = self.project_tracking.year;
                if self.client.is_some() && !self.project_tracking.budgets.budgets_for(year).is_empty() {
                    self.loading = true;
                    self.project_tracking_gen += 1;
                    self.load_project_tracking_task()
                } else {
                    self.recompute_project_tracking_summaries();
                    Task::none()
                }
            }

            ProjectTrackingMsg::Refresh => {
                self.project_tracking.entries.clear();
                self.project_tracking.entries.shrink_to_fit();
                self.project_tracking.summaries.clear();
                let year = self.project_tracking.year;
                if self.client.is_some() && !self.project_tracking.budgets.budgets_for(year).is_empty() {
                    self.loading = true;
                    self.project_tracking_gen += 1;
                    self.load_project_tracking_task()
                } else {
                    self.recompute_project_tracking_summaries();
                    Task::none()
                }
            }

            ProjectTrackingMsg::EntriesLoaded(gen, result) => {
                if gen != self.project_tracking_gen { return Task::none(); }
                self.loading = false;
                match result {
                    Ok(entries) => {
                        self.project_tracking.entries = entries;
                        self.recompute_project_tracking_summaries();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            ProjectTrackingMsg::ShowForm => {
                self.project_tracking.form = Some(BudgetForm::default());
                Task::none()
            }

            ProjectTrackingMsg::HideForm => {
                self.project_tracking.form = None;
                Task::none()
            }

            ProjectTrackingMsg::EditBudget(id) => {
                let year = self.project_tracking.year;
                if let Some(budget) = self.project_tracking.budgets.budgets_for(year).iter().find(|b| b.id == id) {
                    let selected_projects: Vec<(i64, String, String)> = budget.project_ids.iter().filter_map(|&pid| {
                        self.assignments.iter().find(|a| a.project.id == pid).map(|a| {
                            (a.project.id, a.project.name.clone(), a.client.name.clone())
                        })
                    }).collect();

                    self.project_tracking.form = Some(BudgetForm {
                        name_input: budget.name.clone(),
                        budget_hours_input: format!("{}", budget.budget_hours),
                        project_query: String::new(),
                        selected_projects,
                        editing_id: Some(id),
                        error: None,
                    });
                }
                Task::none()
            }

            ProjectTrackingMsg::DeleteBudget(id) => {
                let year = self.project_tracking.year;
                self.project_tracking.budgets.budgets_for_mut(year).retain(|b| b.id != id);
                if let Err(e) = self.project_tracking.budgets.save(&self.settings.data_dir) {
                    self.error_banner = Some(format!("Failed to save budgets: {e}"));
                }
                self.recompute_project_tracking_summaries();
                Task::none()
            }

            ProjectTrackingMsg::NameChanged(v) => {
                if let Some(f) = &mut self.project_tracking.form { f.name_input = v; f.error = None; }
                Task::none()
            }

            ProjectTrackingMsg::BudgetHoursChanged(v) => {
                if let Some(f) = &mut self.project_tracking.form { f.budget_hours_input = v; f.error = None; }
                Task::none()
            }

            ProjectTrackingMsg::ProjectQueryChanged(v) => {
                if let Some(f) = &mut self.project_tracking.form { f.project_query = v; }
                Task::none()
            }

            ProjectTrackingMsg::ProjectSelected(idx) => {
                if let Some(f) = &mut self.project_tracking.form {
                    if let Some(a) = self.assignments.iter().filter(|a| a.is_active).nth(idx) {
                        let pid = a.project.id;
                        if !f.selected_projects.iter().any(|(id, _, _)| *id == pid) {
                            f.selected_projects.push((pid, a.project.name.clone(), a.client.name.clone()));
                        }
                    }
                    f.project_query.clear();
                    f.error = None;
                }
                Task::none()
            }

            ProjectTrackingMsg::ProjectRemoved(pid) => {
                if let Some(f) = &mut self.project_tracking.form {
                    f.selected_projects.retain(|(id, _, _)| *id != pid);
                }
                Task::none()
            }

            ProjectTrackingMsg::FormSubmit => {
                let Some(form) = &self.project_tracking.form else { return Task::none(); };

                let validated = match form.validate() {
                    Ok(v) => v,
                    Err(e) => {
                        if let Some(f) = &mut self.project_tracking.form { f.error = Some(e); }
                        return Task::none();
                    }
                };

                let year = self.project_tracking.year;
                let is_new = validated.editing_id.is_none();

                if let Some(id) = validated.editing_id {
                    if let Some(budget) = self.project_tracking.budgets.budgets_for_mut(year).iter_mut().find(|b| b.id == id) {
                        budget.name = validated.name;
                        budget.budget_hours = validated.budget_hours;
                        budget.project_ids = validated.project_ids;
                    }
                } else {
                    let id = self.project_tracking.budgets.next_id;
                    self.project_tracking.budgets.next_id += 1;
                    self.project_tracking.budgets.budgets_for_mut(year).push(ProjectBudget {
                        id,
                        name: validated.name,
                        budget_hours: validated.budget_hours,
                        project_ids: validated.project_ids,
                        task_ids: Vec::new(),
                    });
                }

                if let Err(e) = self.project_tracking.budgets.save(&self.settings.data_dir) {
                    // Roll back in-memory change and keep the form open so the user can retry.
                    if is_new {
                        self.project_tracking.budgets.budgets_for_mut(year).pop();
                        self.project_tracking.budgets.next_id -= 1;
                    }
                    // For edits the budget fields were mutated in-place; we can't easily
                    // restore them without cloning the old state up-front, so we surface the
                    // error and let the user correct via the still-open form.
                    self.error_banner = Some(format!("Failed to save budgets: {e}"));
                    return Task::none();
                }

                self.project_tracking.form = None;

                if self.client.is_some() && !self.project_tracking.budgets.budgets_for(year).is_empty() {
                    self.loading = true;
                    self.project_tracking_gen += 1;
                    self.load_project_tracking_task()
                } else {
                    self.recompute_project_tracking_summaries();
                    Task::none()
                }
            }
        }
    }
}
