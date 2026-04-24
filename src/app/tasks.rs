use super::*;

// ── Tasks ─────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    /// Fetch the current user's Harvest ID and store it in `harvest_user_id`.
    /// This must be resolved before any time-entry listing task so that
    /// manager-role accounts do not receive other users' entries.
    pub(super) fn load_current_user_task(&self) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                client
                    .get_current_user()
                    .await
                    .map(|u| u.id)
                    .map_err(|e| e.to_string())
            },
            Message::CurrentUserLoaded,
        )
    }

    pub(super) fn load_entries_task(&self) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        let date = self.current_date.format("%Y-%m-%d").to_string();
        let gen = self.entries_gen;
        let user_id = self.harvest_user_id;
        Task::perform(
            async move {
                client
                    .list_all_time_entries(user_id, &date, &date)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::Entry(Box::new(EntryMsg::Loaded(gen, result))),
        )
    }

    pub(super) fn load_assignments_task(&self) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        let data_dir = self.settings.data_dir.clone();
        Task::perform(
            async move {
                // Try cache first (24-hour TTL)
                if let Some(cache) = ProjectCache::load(&data_dir) {
                    if cache.is_valid() {
                        return Ok(cache.assignments);
                    }
                }
                let assignments = client
                    .list_all_my_project_assignments()
                    .await
                    .map_err(|e| e.to_string())?;
                // Best-effort cache write
                let _ = ProjectCache::new(assignments.clone()).save(&data_dir);
                Ok(assignments)
            },
            |result| Message::Entry(Box::new(EntryMsg::AssignmentsLoaded(result))),
        )
    }

    pub(super) fn load_stats_task(&self) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        let today = Local::now().naive_local().date();
        let year = self.overtime_year;
        let from = format!("{year}-01-01");
        let to = format!("{year}-12-31");
        // For past years use Dec 31 as the balance end date so all year entries count.
        let balance_end = if year < today.year() {
            chrono::NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today)
        } else {
            today
        };
        let balance_end_str = balance_end.format("%Y-%m-%d").to_string();
        let expected_per_day = self.settings.expected_hours_per_day();
        let public_holidays = swiss_public_holidays(year);
        let carryover = self.settings.overtime_carryover_for(year);
        let holiday_task_ids = self.settings.holiday_task_ids.clone();
        let total_holiday_days = self.settings.effective_holiday_days_for(year);
        let first_work_day = self.settings.first_work_day;
        let adj_total = self.overtime_adjustments.adjustments_total(year);
        let gen = self.stats_gen;
        let user_id = self.harvest_user_id;

        Task::perform(
            async move {
                let all_entries = client
                    .list_all_time_entries(user_id, &from, &to)
                    .await
                    .map_err(|e| e.to_string())?;

                // Balance only counts entries up to balance_end (today for current year,
                // Dec 31 for past years); holiday stats need the full year.
                let ytd_entries: Vec<_> = all_entries
                    .iter()
                    .filter(|e| e.spent_date.as_str() <= balance_end_str.as_str())
                    .cloned()
                    .collect();

                // Use first_work_day as effective start when it falls in the target year so
                // expected hours are not inflated by months the user wasn't employed yet.
                let effective_start = first_work_day.filter(|d| d.year() == year);
                let balance = year_to_date_balance(
                    &ytd_entries,
                    year,
                    effective_start,
                    expected_per_day,
                    &public_holidays,
                    carryover,
                    adj_total,
                    balance_end,
                );
                let holidays = crate::stats::holiday_stats(
                    &all_entries,
                    year,
                    &holiday_task_ids,
                    total_holiday_days,
                    expected_per_day,
                );
                let months = crate::stats::month_summaries(
                    &all_entries,
                    year,
                    effective_start,
                    expected_per_day,
                    &public_holidays,
                    balance_end,
                );
                Ok((balance, holidays, months))
            },
            move |result| Message::Stats(StatsMsg::Loaded(gen, result)),
        )
    }

    pub(super) fn load_vacation_task(&self) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        let year = self.vacation.year;
        let from = format!("{year}-01-01");
        let to = format!("{year}-12-31");
        let gen = self.vacation_gen;
        let user_id = self.harvest_user_id;
        Task::perform(
            async move {
                client
                    .list_all_time_entries(user_id, &from, &to)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::Vacation(VacationMsg::EntriesLoaded(gen, result)),
        )
    }

    pub(super) fn load_billable_task(&self) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        let year = self.billable.year;
        let (from, to) = match self.billable.month {
            None => (format!("{year}-01-01"), format!("{year}-12-31")),
            Some(m) => {
                let first = NaiveDate::from_ymd_opt(year, m, 1)
                    .expect("month is 1-12, enforced by UI month picker");
                let last = if m == 12 {
                    NaiveDate::from_ymd_opt(year + 1, 1, 1)
                        .expect("year+1 is always a valid year")
                } else {
                    NaiveDate::from_ymd_opt(year, m + 1, 1)
                        .expect("month+1 is 2-12, always valid")
                }
                .pred_opt()
                .expect("Jan 1 of any year always has a predecessor");
                (first.format("%Y-%m-%d").to_string(), last.format("%Y-%m-%d").to_string())
            }
        };
        let gen = self.billable_gen;
        let user_id = self.harvest_user_id;
        Task::perform(
            async move {
                client
                    .list_all_time_entries(user_id, &from, &to)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::Billable(BillableMsg::EntriesLoaded(gen, result)),
        )
    }

    pub(super) fn submit_vacation_task(
        &self,
        entries: Vec<crate::harvest::models::CreateTimeEntry>,
    ) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let mut created = Vec::new();
                for entry in entries {
                    match client.create_time_entry(&entry).await {
                        Ok(e) => created.push(e),
                        Err(e) => return Err(e.to_string()),
                    }
                }
                Ok(created)
            },
            |result| Message::Vacation(VacationMsg::EntriesCreated(result)),
        )
    }

    /// Reload the work day store if `current_date` has crossed into a new month.
    pub(super) fn maybe_reload_work_day_store(&mut self) {
        let y = self.current_date.year();
        let m = self.current_date.month();
        if self.work_day_store.year != y || self.work_day_store.month != m {
            self.work_day_store = WorkDayStore::load(&self.settings.data_dir, y, m);
        }
    }

    /// Update the tray's phase so the context menu shows the correct actions.
    #[cfg(not(target_os = "macos"))]
    pub(super) fn sync_tray_phase(&self) {
        let today = Local::now().naive_local().date();
        let day = self.work_day_store.get_or_default(today);
        if let Ok(mut lock) = self.tray_phase.lock() {
            *lock = day.phase();
        }
        self.tray_update_notify.notify_one();
    }

    pub(super) fn recompute_billable_summary(&mut self) {
        let entries = &self.billable.entries;
        let total_hours: f64 = entries.iter().map(|e| e.hours).sum();
        let billable_hours: f64 = entries.iter().filter(|e| e.billable).map(|e| e.hours).sum();
        let non_billable_hours = total_hours - billable_hours;
        let billable_pct = if total_hours > 0.0 { billable_hours / total_hours } else { 0.0 };

        // Group by project
        use std::collections::HashMap;
        let mut map: HashMap<i64, (&str, &str, f64, f64)> = HashMap::new();
        for e in entries {
            let rec = map.entry(e.project.id).or_insert((&e.project.name, &e.client.name, 0.0, 0.0));
            rec.3 += e.hours;
            if e.billable { rec.2 += e.hours; }
        }
        let mut projects: Vec<(String, String, f64, f64)> = map
            .into_values()
            .map(|(name, client, b, t)| (name.to_owned(), client.to_owned(), b, t))
            .collect();
        projects.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        self.billable.summary = Some(BillableSummary {
            total_hours, billable_hours, non_billable_hours, billable_pct, projects,
        });
    }

    pub(super) fn recompute_vacation_summary(&mut self) {
        let expected_per_day = self.settings.expected_hours_per_day();
        let task_ids = &self.settings.holiday_task_ids;
        let today = chrono::Local::now().naive_local().date();
        let year = self.vacation.year;

        // Single pass: parse the date once per entry and bucket into used vs booked.
        let entries = &self.vacation.entries;
        let (used_days, booked_days) = entries
            .iter()
            .filter(|e| task_ids.contains(&e.task.id))
            .filter_map(|e| {
                NaiveDate::parse_from_str(&e.spent_date, "%Y-%m-%d")
                    .ok()
                    .map(|d| (d, e.hours / expected_per_day))
            })
            .fold((0.0_f64, 0.0_f64), |(used, booked), (d, days)| {
                if d <= today { (used + days, booked) } else { (used, booked + days) }
            });

        let total_days = self.settings.effective_holiday_days_for(year);
        let days_remaining = total_days - used_days - booked_days;
        let carryover_days = self
            .settings
            .carryover
            .get(&year)
            .map(|c| c.holiday_days)
            .unwrap_or(0.0);

        self.vacation.summary = Some(VacationSummary {
            used_days, booked_days, days_remaining, total_days, carryover_days,
        });
    }

    pub(super) fn recompute_project_options(&mut self) {
        self.cached_project_options = self.favorites.sorted_options(&self.assignments);
    }

    pub(super) fn recompute_expected_hours(&mut self) {
        let d = self.current_date;
        let wd = d.weekday().num_days_from_monday();
        if wd >= 5 {
            self.cached_expected_hours = 0.0;
            return;
        }
        let epd = self.settings.expected_hours_per_day();
        for h in &crate::state::settings::swiss_public_holidays(d.year()) {
            if h.date == d {
                self.cached_expected_hours = epd - h.credit_hours(epd);
                return;
            }
        }
        self.cached_expected_hours = epd;
    }

    pub(super) fn recompute_task_list(&mut self) {
        let mut tasks: Vec<(i64, String, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for pa in self.assignments.iter().filter(|p| p.is_active) {
            for ta in pa.task_assignments.iter().filter(|t| t.is_active) {
                if seen.insert(ta.task.id) {
                    tasks.push((
                        ta.task.id,
                        ta.task.name.clone(),
                        format!("{} — {}", pa.client.name, pa.project.name),
                    ));
                }
            }
        }
        tasks.sort_by(|a, b| a.1.cmp(&b.1));
        self.settings_form.cached_task_list = tasks;
    }

    pub(super) fn load_project_tracking_task(&self) -> Task<Message> {
        use super::project_tracking::ProjectTrackingMsg;
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        let year = self.project_tracking.year;
        let gen = self.project_tracking_gen;

        let from = format!("{year}-01-01");
        let today = Local::now().naive_local().date();
        let to = if year < today.year() {
            format!("{year}-12-31")
        } else {
            today.format("%Y-%m-%d").to_string()
        };

        // Collect all project IDs from this year's budgets.
        let all_project_ids: std::collections::HashSet<i64> = self
            .project_tracking
            .budgets
            .budgets_for(year)
            .iter()
            .flat_map(|b| b.project_ids.iter().copied())
            .collect();

        let user_id = self.harvest_user_id;
        Task::perform(
            async move {
                let all_entries = client
                    .list_all_time_entries(user_id, &from, &to)
                    .await
                    .map_err(|e| e.to_string())?;
                let filtered: Vec<_> = all_entries
                    .into_iter()
                    .filter(|e| all_project_ids.contains(&e.project.id))
                    .collect();
                Ok(filtered)
            },
            move |result| Message::ProjectTracking(ProjectTrackingMsg::EntriesLoaded(gen, result)),
        )
    }

    pub(super) fn recompute_project_tracking_summaries(&mut self) {
        let year = self.project_tracking.year;
        let entries = &self.project_tracking.entries;
        self.project_tracking.summaries = compute_budget_summaries(
            self.project_tracking.budgets.budgets_for(year),
            entries,
        );
    }
}

// ── Budget summary computation (pure, testable) ────────────────────────────

pub(super) fn compute_budget_summaries(
    budgets: &[crate::state::project_budgets::ProjectBudget],
    entries: &[crate::harvest::models::TimeEntry],
) -> Vec<super::project_tracking::BudgetSummary> {
    budgets
        .iter()
        .map(|budget| {
            let used_hours: f64 = entries
                .iter()
                .filter(|e| budget.project_ids.contains(&e.project.id))
                .filter(|e| budget.task_ids.is_empty() || budget.task_ids.contains(&e.task.id))
                .map(|e| e.hours)
                .sum();
            let remaining_hours = budget.budget_hours - used_hours;
            let pct_used = if budget.budget_hours > 0.0 {
                used_hours / budget.budget_hours
            } else {
                0.0
            };
            super::project_tracking::BudgetSummary {
                budget: budget.clone(),
                used_hours,
                remaining_hours,
                pct_used,
            }
        })
        .collect()
}

// ── Vacation entry builder (pure, testable) ─────────────────────────────────

pub(super) fn build_vacation_entries(
    from: NaiveDate,
    to: NaiveDate,
    vacation_year: i32,
    hours: f64,
    project_id: i64,
    task_id: i64,
) -> Result<Vec<crate::harvest::models::CreateTimeEntry>, String> {
    use chrono::Weekday;

    if from.year() != vacation_year || to.year() != vacation_year {
        return Err(format!("Dates must be in the year {vacation_year}."));
    }
    if from > to {
        return Err("From date must be before or equal to To date.".into());
    }

    let holidays = swiss_public_holidays(from.year());
    let mut entries = Vec::new();
    let mut d = from;
    while d <= to {
        let is_weekend = matches!(d.weekday(), Weekday::Sat | Weekday::Sun);
        let is_holiday = holidays.iter().any(|h| h.date == d);
        if !is_weekend && !is_holiday {
            entries.push(crate::harvest::models::CreateTimeEntry {
                project_id,
                task_id,
                spent_date: d.format("%Y-%m-%d").to_string(),
                hours,
                notes: None,
            });
        }
        d += chrono::Duration::days(1);
    }
    if entries.is_empty() {
        return Err("No workdays in selected range (weekends and holidays excluded).".into());
    }
    Ok(entries)
}

pub(super) fn format_harvest_error(e: HarvestError) -> String {
    match e {
        HarvestError::Api { status, body } => {
            format!("API error {status}: {body}")
        }
        HarvestError::Http(e) => format!("Network error: {e}"),
        HarvestError::RateLimited { retry_after_secs } => {
            format!("Rate limited — retry after {retry_after_secs}s")
        }
        HarvestError::Unauthorized => {
            "Authentication failed. Please check your API token and Account ID.".into()
        }
    }
}
