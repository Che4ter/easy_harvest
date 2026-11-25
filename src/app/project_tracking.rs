use super::*;
use std::collections::HashMap;

use crate::harvest::models::ProjectBudgetReport;
use crate::state::project_budgets::{BudgetScope, ProjectBudget, ProjectBudgetStore};
use crate::state::project_tracking_layout::{CardKey, ProjectTrackingLayout};

// ── Page state ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BudgetSummary {
    pub budget: ProjectBudget,
    pub used_hours: f64,
    pub remaining_hours: f64,
    pub pct_used: f64,
    pub split: super::PersonSplit,
}

#[derive(Debug, Clone, Default)]
pub struct BudgetForm {
    pub name_input: String,
    pub budget_hours_input: String,
    pub project_query: String,
    /// Selected (project_id, project_name, client_name) tuples.
    pub selected_projects: Vec<(i64, String, String)>,
    pub editing_id: Option<u64>,
    pub scope: BudgetScope,
    pub error: Option<String>,
}

pub struct ValidatedBudget {
    pub name: String,
    pub budget_hours: f64,
    pub project_ids: Vec<i64>,
    pub scope: BudgetScope,
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
            // M5-F2: also exclude non-finite values (Infinity, NaN) — they pass the
            // `v > 0.0` check but corrupt budget-remaining calculations downstream.
            .filter(|&v: &f64| v.is_finite() && v > 0.0)
            .ok_or_else(|| "Enter a valid positive number for budget hours.".to_string())?;
        if self.selected_projects.is_empty() {
            return Err("Select at least one project.".into());
        }
        let project_ids = self.selected_projects.iter().map(|(id, _, _)| *id).collect();
        Ok(ValidatedBudget { name, budget_hours, project_ids, scope: self.scope, editing_id: self.editing_id })
    }
}

/// Per-person split of a managed budget card, loaded separately from totals.
#[derive(Debug, Clone)]
pub enum SplitState {
    Loading,
    Ready(super::PersonSplit),
    Failed,
}

/// An in-progress card drag. `from` is the dragged card (a key, so cards
/// loading mid-drag can't shift it); `over` is the insertion slot in the
/// visible ordered card list (0..=len), `None` while outside the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragState {
    pub from: CardKey,
    pub over: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTrackingPageState {
    pub budgets: ProjectBudgetStore,
    pub year: i32,
    pub entries: Vec<TimeEntry>,
    pub summaries: Vec<BudgetSummary>,
    pub form: Option<BudgetForm>,
    /// Harvest hour budgets of projects the user manages (year-independent).
    pub managed: Vec<super::ManagedBudget>,
    pub managed_splits: HashMap<i64, SplitState>,
    pub managed_loading: bool,
    /// Generation for managed loads; separate from `project_tracking_gen` so
    /// year switches (which reload manual entries) don't drop managed results.
    pub managed_gen: u64,
    pub layout: ProjectTrackingLayout,
    pub show_hidden: bool,
    pub drag: Option<DragState>,
}

impl ProjectTrackingPageState {
    pub fn new(data_dir: &std::path::Path, year: i32) -> Self {
        Self {
            budgets: ProjectBudgetStore::load(data_dir),
            year,
            layout: ProjectTrackingLayout::load(data_dir),
            ..Default::default()
        }
    }
}

/// Title of a managed card: `"{project} · {client}"`.
pub(crate) fn managed_card_title(m: &super::ManagedBudget) -> String {
    format!("{} · {}", m.project_name, m.client_name)
}

/// Card keys of the Projects page in display order (visible and hidden):
/// managed projects plus manual budgets of the selected year. Unordered cards
/// are sorted by their title.
fn ordered_cards(state: &ProjectTrackingPageState) -> Vec<CardKey> {
    let present: Vec<(CardKey, String)> = state
        .managed
        .iter()
        .map(|m| (CardKey::Harvest(m.project_id), managed_card_title(m)))
        .chain(
            state
                .budgets
                .budgets_for(state.year)
                .iter()
                .map(|b| (CardKey::Manual(b.id), b.name.clone())),
        )
        .collect();
    state.layout.ordered(&present)
}

/// Card keys of the Projects page in display order, split into visible and
/// hidden.
pub(crate) fn visible_and_hidden_cards(
    state: &ProjectTrackingPageState,
) -> (Vec<CardKey>, Vec<CardKey>) {
    ordered_cards(state)
        .into_iter()
        .partition(|k| !state.layout.is_hidden(*k))
}

/// Maps a move of visible card `from` to visible insertion slot `to`
/// (0..=visible.len()) onto `(from, to)` indices of `full`, the visible and
/// hidden cards in display order. Slot `visible.len()` maps to right after
/// the last visible card, so hidden cards keep their relative positions.
pub(crate) fn full_list_move(
    full: &[CardKey],
    visible: &[CardKey],
    from: usize,
    to: usize,
) -> Option<(usize, usize)> {
    let position = |key: &CardKey| full.iter().position(|k| k == key);
    let full_from = position(visible.get(from)?)?;
    let full_to = if to == visible.len() {
        position(visible.last()?)? + 1
    } else {
        position(visible.get(to)?)?
    };
    Some((full_from, full_to))
}

// ── Messages ────────────────────────────────────────────────────────────────

/// Raw inputs for managed budgets: the user's project assignments and the
/// project budget report.
pub type ManagedData = (Vec<ProjectAssignment>, Vec<ProjectBudgetReport>);

#[derive(Debug, Clone)]
pub enum ProjectTrackingMsg {
    Refresh,
    YearPrev,
    YearNext,
    EntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    /// `Ok(None)`: the user may not read budget reports (HTTP 403).
    ManagedLoaded(u64, Result<Option<ManagedData>, String>),
    SplitLoaded(u64, i64, Result<super::PersonSplit, String>),
    HideCard(CardKey),
    UnhideCard(CardKey),
    ToggleShowHidden,
    /// Drag of this card started (handle pressed).
    DragStart(CardKey),
    /// Cursor entered insertion slot (0..=visible len) during a drag.
    DragOver(usize),
    /// Cursor left the card list during a drag.
    DragLeave,
    /// Mouse released: move the card to the current slot, if any.
    DragDrop,
    DragCancel,
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
    ScopeChanged(BudgetScope),
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
                let manual = if self.client.is_some() && !self.project_tracking.budgets.budgets_for(year).is_empty() {
                    self.loading = true;
                    self.project_tracking_gen += 1;
                    self.load_project_tracking_task()
                } else {
                    self.recompute_project_tracking_summaries();
                    Task::none()
                };
                Task::batch([manual, self.start_managed_budgets_load()])
            }

            ProjectTrackingMsg::EntriesLoaded(r#gen, result) => {
                if r#gen != self.project_tracking_gen { return Task::none(); }
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

            ProjectTrackingMsg::ManagedLoaded(r#gen, result) => {
                if r#gen != self.project_tracking.managed_gen { return Task::none(); }
                self.project_tracking.managed_loading = false;
                match result {
                    Ok(Some((assignments, reports))) => {
                        let managed = super::budget_split::managed_hour_budgets(&assignments, &reports);
                        self.project_tracking.managed_splits = managed
                            .iter()
                            .map(|m| (m.project_id, SplitState::Loading))
                            .collect();
                        self.project_tracking.managed = managed;
                        self.prune_project_tracking_layout();
                        self.load_managed_splits_task()
                    }
                    Ok(None) => {
                        self.project_tracking.managed.clear();
                        self.project_tracking.managed_splits.clear();
                        Task::none()
                    }
                    Err(e) => {
                        self.error_banner = Some(e);
                        Task::none()
                    }
                }
            }

            ProjectTrackingMsg::SplitLoaded(r#gen, project_id, result) => {
                if r#gen != self.project_tracking.managed_gen { return Task::none(); }
                let state = match result {
                    Ok(split) => SplitState::Ready(split),
                    Err(_) => SplitState::Failed,
                };
                self.project_tracking.managed_splits.insert(project_id, state);
                Task::none()
            }

            ProjectTrackingMsg::HideCard(key) => {
                self.set_card_hidden(key, true);
                Task::none()
            }

            ProjectTrackingMsg::UnhideCard(key) => {
                self.set_card_hidden(key, false);
                Task::none()
            }

            ProjectTrackingMsg::ToggleShowHidden => {
                self.project_tracking.show_hidden = !self.project_tracking.show_hidden;
                Task::none()
            }

            ProjectTrackingMsg::DragStart(from) => {
                self.project_tracking.drag = Some(DragState { from, over: None });
                Task::none()
            }

            ProjectTrackingMsg::DragOver(slot) => {
                if let Some(drag) = &mut self.project_tracking.drag {
                    drag.over = Some(slot);
                }
                Task::none()
            }

            ProjectTrackingMsg::DragLeave => {
                if let Some(drag) = &mut self.project_tracking.drag {
                    drag.over = None;
                }
                Task::none()
            }

            ProjectTrackingMsg::DragDrop => {
                // Released outside the list (`over == None`) acts as a cancel.
                if let Some(DragState { from, over: Some(to) }) = self.project_tracking.drag.take() {
                    self.move_visible_card(from, to);
                }
                Task::none()
            }

            ProjectTrackingMsg::DragCancel => {
                self.project_tracking.drag = None;
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
                        scope: budget.scope,
                        error: None,
                    });
                }
                Task::none()
            }

            ProjectTrackingMsg::DeleteBudget(id) => {
                let year = self.project_tracking.year;
                // Stash the item before removing so we can roll back on save failure.
                let removed: Vec<_> = self.project_tracking.budgets
                    .budgets_for(year)
                    .iter()
                    .filter(|b| b.id == id)
                    .cloned()
                    .collect();
                self.project_tracking.budgets.budgets_for_mut(year).retain(|b| b.id != id);
                if let Err(e) = self.project_tracking.budgets.save(&self.settings.data_dir) {
                    // Roll back so displayed data matches disk.
                    self.project_tracking.budgets.budgets_for_mut(year).extend(removed);
                    self.error_banner = Some(format!("Failed to save budgets: {e}"));
                    return Task::none();
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

            ProjectTrackingMsg::ScopeChanged(scope) => {
                if let Some(f) = &mut self.project_tracking.form { f.scope = scope; }
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
                        budget.scope = validated.scope;
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
                        scope: validated.scope,
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

impl EasyHarvest {
    /// Bumps the managed generation and starts loading managed budgets when a
    /// client is configured. Manual budgets are irrelevant to this load.
    pub(super) fn start_managed_budgets_load(&mut self) -> Task<Message> {
        if self.client.is_none() {
            return Task::none();
        }
        self.project_tracking.managed_gen += 1;
        self.project_tracking.managed_loading = true;
        self.load_managed_budgets_task()
    }

    /// Drops layout keys of cards that no longer exist: managed projects plus
    /// manual budgets of every year (so other years keep their order/hidden).
    fn prune_project_tracking_layout(&mut self) {
        let pt = &mut self.project_tracking;
        let present: Vec<CardKey> = pt
            .managed
            .iter()
            .map(|m| CardKey::Harvest(m.project_id))
            .chain(pt.budgets.years.values().flatten().map(|b| CardKey::Manual(b.id)))
            .collect();
        pt.layout.prune(&present);
    }

    /// Moves card `key` to visible insertion slot `to` and saves the layout.
    /// Hidden cards keep their relative positions. Does nothing when the card
    /// is no longer visible (e.g. its data reloaded mid-drag).
    fn move_visible_card(&mut self, key: CardKey, to: usize) {
        let full = ordered_cards(&self.project_tracking);
        let visible: Vec<CardKey> = full
            .iter()
            .copied()
            .filter(|k| !self.project_tracking.layout.is_hidden(*k))
            .collect();
        let Some(from) = visible.iter().position(|k| *k == key) else {
            return;
        };
        let Some((full_from, full_to)) = full_list_move(&full, &visible, from, to) else {
            return;
        };
        if full_to == full_from || full_to == full_from + 1 {
            return; // Dropped in place: nothing to save.
        }
        let previous = self.project_tracking.layout.clone();
        self.project_tracking.layout.move_card(&full, full_from, full_to);
        if let Err(e) = self.project_tracking.layout.save(&self.settings.data_dir) {
            self.project_tracking.layout = previous;
            self.error_banner = Some(format!("Failed to save project layout: {e}"));
        }
    }

    fn set_card_hidden(&mut self, key: CardKey, hidden: bool) {
        let previous = self.project_tracking.layout.clone();
        self.project_tracking.layout.set_hidden(key, hidden);
        if let Err(e) = self.project_tracking.layout.save(&self.settings.data_dir) {
            self.project_tracking.layout = previous;
            self.error_banner = Some(format!("Failed to save project layout: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn managed(project_id: i64, project_name: &str) -> super::super::ManagedBudget {
        super::super::ManagedBudget {
            project_id,
            project_name: project_name.to_string(),
            client_name: "Client".to_string(),
            budget_hours: 100.0,
            spent_hours: 10.0,
            remaining_hours: 90.0,
            is_monthly: false,
            budget_by: "project".to_string(),
        }
    }

    fn manual(id: u64, name: &str) -> ProjectBudget {
        ProjectBudget {
            id,
            name: name.to_string(),
            budget_hours: 50.0,
            project_ids: vec![1],
            task_ids: Vec::new(),
            scope: BudgetScope::Me,
        }
    }

    fn state() -> ProjectTrackingPageState {
        let mut s = ProjectTrackingPageState { year: 2026, ..Default::default() };
        s.managed = vec![managed(10, "Zeta"), managed(11, "Alpha")];
        s.budgets.budgets_for_mut(2026).push(manual(1, "Middle"));
        s
    }

    #[test]
    fn cards_sorted_by_title_without_stored_order() {
        let (visible, hidden) = visible_and_hidden_cards(&state());
        assert_eq!(
            visible,
            vec![CardKey::Harvest(11), CardKey::Manual(1), CardKey::Harvest(10)]
        );
        assert!(hidden.is_empty());
    }

    #[test]
    fn cards_follow_stored_order_and_hidden_are_split_out() {
        let mut s = state();
        s.layout.order = vec![CardKey::Harvest(10), CardKey::Manual(1)];
        s.layout.set_hidden(CardKey::Manual(1), true);
        let (visible, hidden) = visible_and_hidden_cards(&s);
        assert_eq!(visible, vec![CardKey::Harvest(10), CardKey::Harvest(11)]);
        assert_eq!(hidden, vec![CardKey::Manual(1)]);
    }

    #[test]
    fn manual_budgets_of_other_years_are_excluded() {
        let mut s = state();
        s.budgets.budgets_for_mut(2025).push(manual(2, "Old"));
        s.layout.set_hidden(CardKey::Manual(2), true);
        let (visible, hidden) = visible_and_hidden_cards(&s);
        assert!(!visible.contains(&CardKey::Manual(2)));
        assert!(hidden.is_empty());
    }
}
