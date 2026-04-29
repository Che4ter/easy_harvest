use super::*;

// ── Overtime adjustment form ────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct OvertimeAdjustmentForm {
    pub date_input: String,
    pub hours_input: String,
    pub reason_input: String,
    pub error: Option<String>,
}

pub struct ValidatedAdjustment {
    pub date: NaiveDate,
    pub hours: f64,
    pub reason: String,
}

impl OvertimeAdjustmentForm {
    pub fn validate(&self, expected_year: i32) -> Result<ValidatedAdjustment, String> {
        let date = NaiveDate::parse_from_str(self.date_input.trim(), "%d.%m.%Y")
            .or_else(|_| NaiveDate::parse_from_str(self.date_input.trim(), "%Y-%m-%d"))
            .map_err(|_| "Enter a valid date (DD.MM.YYYY).".to_string())?;
        if date.year() != expected_year {
            return Err(format!("Date must be in year {expected_year}."));
        }
        let hours: f64 = self.hours_input.replace(',', ".").parse()
            .ok()
            .filter(|&v: &f64| v != 0.0)
            .ok_or_else(|| "Enter a non-zero number of hours (negative to subtract).".to_string())?;
        let reason = self.reason_input.trim().to_string();
        if reason.is_empty() {
            return Err("Enter a reason for the adjustment.".into());
        }
        Ok(ValidatedAdjustment { date, hours, reason })
    }
}

// ── Stats messages ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StatsMsg {
    Refresh,
    YearPrev,
    YearNext,
    Loaded(u64, Result<(YearBalance, HolidayStats, Vec<crate::stats::MonthSummary>), String>),
    // Adjustment form
    ShowAdjForm,
    HideAdjForm,
    AdjDateChanged(String),
    AdjHoursChanged(String),
    AdjReasonChanged(String),
    AdjSubmit,
    AdjDelete(u64),
}

impl EasyHarvest {
    pub(super) fn update_stats(&mut self, msg: StatsMsg) -> Task<Message> {
        match msg {
            StatsMsg::Refresh => {
                self.loading = true;
                self.year_balance = None;
                self.holiday_stats = None;
                self.month_summaries = None;
                self.stats_gen += 1;
                self.load_stats_task()
            }

            StatsMsg::YearPrev => {
                self.overtime_year -= 1;
                self.year_balance = None;
                self.holiday_stats = None;
                self.month_summaries = None;
                self.overtime_adj_form = None;
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
                self.month_summaries = None;
                self.overtime_adj_form = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.stats_gen += 1;
                    self.load_stats_task()
                } else {
                    Task::none()
                }
            }

            StatsMsg::Loaded(r#gen, result) => {
                if r#gen != self.stats_gen { return Task::none(); }
                self.loading = false;
                match result {
                    Ok((balance, holidays, months)) => {
                        self.year_balance = Some(balance);
                        self.holiday_stats = Some(holidays);
                        self.month_summaries = Some(months);
                        // Persist derived carryover into settings.json so the next year
                        // picks it up automatically.  Past years are immutable so this
                        // only needs to run once; existing (manual) entries are preserved.
                        let year = self.overtime_year;
                        if year < Local::now().naive_local().date().year() {
                            let next = year + 1;
                            if !self.settings.carryover.contains_key(&next) {
                                if let (Some(bal), Some(hols)) =
                                    (&self.year_balance, &self.holiday_stats)
                                {
                                    let epd = self.settings.expected_hours_per_day();
                                    self.settings.carryover.insert(
                                        next,
                                        crate::state::settings::YearCarryover {
                                            overtime_hours: bal.total_balance,
                                            holiday_hours: hols.days_remaining * epd,
                                            ..Default::default()
                                        },
                                    );
                                    let _ = self.settings.save();
                                }
                            }
                        }
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            // ── Adjustment form ─────────────────────────────────────────────
            StatsMsg::ShowAdjForm => {
                self.overtime_adj_form = Some(OvertimeAdjustmentForm::default());
                Task::none()
            }

            StatsMsg::HideAdjForm => {
                self.overtime_adj_form = None;
                Task::none()
            }

            StatsMsg::AdjDateChanged(v) => {
                if let Some(f) = &mut self.overtime_adj_form { f.date_input = v; f.error = None; }
                Task::none()
            }

            StatsMsg::AdjHoursChanged(v) => {
                if let Some(f) = &mut self.overtime_adj_form { f.hours_input = v; f.error = None; }
                Task::none()
            }

            StatsMsg::AdjReasonChanged(v) => {
                if let Some(f) = &mut self.overtime_adj_form { f.reason_input = v; f.error = None; }
                Task::none()
            }

            StatsMsg::AdjSubmit => {
                let Some(form) = &self.overtime_adj_form else { return Task::none(); };

                let validated = match form.validate(self.overtime_year) {
                    Ok(v) => v,
                    Err(e) => {
                        if let Some(f) = &mut self.overtime_adj_form { f.error = Some(e); }
                        return Task::none();
                    }
                };

                let year = self.overtime_year;
                let id = self.overtime_adjustments.next_id;
                self.overtime_adjustments.next_id += 1;
                self.overtime_adjustments.adjustments_for_mut(year).push(
                    crate::state::overtime_adjustments::OvertimeAdjustment {
                        id,
                        date: validated.date.format("%Y-%m-%d").to_string(),
                        hours: validated.hours,
                        reason: validated.reason,
                    }
                );
                if let Err(e) = self.overtime_adjustments.save(&self.settings.data_dir) {
                    // Roll back the in-memory change so displayed data matches disk,
                    // and leave the form open so the user can retry.
                    self.overtime_adjustments.adjustments_for_mut(year).retain(|a| a.id != id);
                    self.overtime_adjustments.next_id -= 1;
                    self.error_banner = Some(format!("Failed to save adjustments: {e}"));
                    return Task::none();
                }
                self.overtime_adj_form = None;

                Task::done(Message::Stats(StatsMsg::Refresh))
            }

            StatsMsg::AdjDelete(id) => {
                let year = self.overtime_year;
                // Stash the item before removing it so we can roll back on save failure.
                let removed: Vec<_> = self.overtime_adjustments
                    .adjustments_for(year)
                    .iter()
                    .filter(|a| a.id == id)
                    .cloned()
                    .collect();
                self.overtime_adjustments.adjustments_for_mut(year).retain(|a| a.id != id);
                if let Err(e) = self.overtime_adjustments.save(&self.settings.data_dir) {
                    // Roll back so displayed data matches disk.
                    self.overtime_adjustments.adjustments_for_mut(year).extend(removed);
                    self.error_banner = Some(format!("Failed to save adjustments: {e}"));
                    return Task::none();
                }
                // Refresh stats to reflect the deletion
                Task::done(Message::Stats(StatsMsg::Refresh))
            }
        }
    }
}
