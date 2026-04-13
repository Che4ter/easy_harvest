use super::*;

// ── Stats ────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(super) fn update_stats(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StatsRefresh => {
                self.loading = true;
                self.year_balance = None;
                self.holiday_stats = None;
                self.stats_gen += 1;
                self.load_stats_task()
            }

            Message::OvertimeYearPrev => {
                self.overtime_year -= 1;
                self.year_balance = None;
                self.holiday_stats = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.stats_gen += 1;
                    self.load_stats_task()
                } else {
                    Task::none()
                }
            }

            Message::OvertimeYearNext => {
                self.overtime_year += 1;
                self.year_balance = None;
                self.holiday_stats = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.stats_gen += 1;
                    self.load_stats_task()
                } else {
                    Task::none()
                }
            }

            _ => unreachable!(),
        }
    }
}
