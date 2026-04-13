use super::*;

// ── Billable ─────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(super) fn update_billable(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::BillableYearPrev => {
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

            Message::BillableYearNext => {
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

            Message::BillableRefresh => {
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

            Message::BillableEntriesLoaded(gen, result) => {
                if gen != self.billable_gen { return Task::none(); }
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

            Message::BillableMonthSelected(m) => {
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

            Message::BillableMonthClear => {
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

            _ => unreachable!(),
        }
    }
}
