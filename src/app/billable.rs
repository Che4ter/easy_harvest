use super::*;

// ── Billable state ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BillableSummary {
    pub total_hours: f64,
    pub billable_hours: f64,
    pub non_billable_hours: f64,
    pub billable_pct: f64,
    pub projects: Vec<(String, String, f64, f64)>, // (project_name, client_name, billable_h, total_h) sorted by billable desc
}

#[derive(Debug, Clone)]
pub struct BillablePageState {
    pub entries: Vec<TimeEntry>,
    pub year: i32,
    /// None = full year view; Some(m) = single month 1–12
    pub month: Option<u32>,
    pub summary: Option<BillableSummary>,
}

impl BillablePageState {
    pub fn new(year: i32) -> Self {
        Self {
            entries: Vec::new(),
            year,
            month: None,
            summary: None,
        }
    }
}

impl Default for BillablePageState {
    fn default() -> Self {
        Self::new(0)
    }
}

// ── Billable messages ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BillableMsg {
    YearPrev,
    YearNext,
    Refresh,
    EntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    MonthSelected(u32),
    MonthClear,
}

impl EasyHarvest {
    pub(super) fn update_billable(&mut self, msg: BillableMsg) -> Task<Message> {
        match msg {
            BillableMsg::YearPrev => {
                self.billable.year -= 1;
                self.billable.month = None;
                self.billable.entries.clear();
                self.billable.entries.shrink_to_fit();
                self.billable.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.billable_gen += 1;
                    self.load_billable_task()
                } else {
                    Task::none()
                }
            }

            BillableMsg::YearNext => {
                self.billable.year += 1;
                self.billable.month = None;
                self.billable.entries.clear();
                self.billable.entries.shrink_to_fit();
                self.billable.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.billable_gen += 1;
                    self.load_billable_task()
                } else {
                    Task::none()
                }
            }

            BillableMsg::Refresh => {
                self.billable.entries.clear();
                self.billable.entries.shrink_to_fit();
                self.billable.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.billable_gen += 1;
                    self.load_billable_task()
                } else {
                    Task::none()
                }
            }

            BillableMsg::EntriesLoaded(r#gen, result) => {
                if r#gen != self.billable_gen { return Task::none(); }
                self.loading = false;
                match result {
                    Ok(entries) => {
                        self.billable.entries = entries;
                        self.recompute_billable_summary();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            BillableMsg::MonthSelected(m) => {
                self.billable.month = Some(m);
                self.billable.entries.clear();
                self.billable.entries.shrink_to_fit();
                self.billable.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.billable_gen += 1;
                    self.load_billable_task()
                } else {
                    Task::none()
                }
            }

            BillableMsg::MonthClear => {
                self.billable.month = None;
                self.billable.entries.clear();
                self.billable.entries.shrink_to_fit();
                self.billable.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.billable_gen += 1;
                    self.load_billable_task()
                } else {
                    Task::none()
                }
            }
        }
    }
}
