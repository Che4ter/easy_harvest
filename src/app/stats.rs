use super::*;

// ── Stats ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StatsMsg {
    Refresh,
    YearPrev,
    YearNext,
    Loaded(u64, Result<(YearBalance, HolidayStats), String>),
}

impl EasyHarvest {
    pub(super) fn update_stats(&mut self, msg: StatsMsg) -> Task<Message> {
        match msg {
            StatsMsg::Refresh => {
                self.loading = true;
                self.year_balance = None;
                self.holiday_stats = None;
                self.stats_gen += 1;
                self.load_stats_task()
            }

            StatsMsg::YearPrev => {
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

            StatsMsg::YearNext => {
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

            StatsMsg::Loaded(gen, result) => {
                if gen != self.stats_gen { return Task::none(); }
                self.loading = false;
                match result {
                    Ok((balance, holidays)) => {
                        self.year_balance = Some(balance);
                        self.holiday_stats = Some(holidays);
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }
        }
    }
}
