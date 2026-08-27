use super::*;
use crate::state::settings::YearCarryover;
use crate::state::team::TeamMember;
use std::collections::HashMap;

// ── Sub-state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TeamMemberForm {
    pub name_input: String,
    pub id_input: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TeamMemberStats {
    pub year_balance: Option<YearBalance>,
    pub holiday_stats: Option<HolidayStats>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct TeamPageState {
    pub add_form: TeamMemberForm,
    pub stats: HashMap<i64, TeamMemberStats>,
    pub adjustment_forms: HashMap<i64, OvertimeAdjustmentForm>,
    pub first_work_day_inputs: HashMap<i64, String>,
    pub work_percentage_inputs: HashMap<i64, String>,
    pub r#gen: u64,
    /// Per-member carryover-sync epoch. Bumped whenever a member's carryover
    /// chain is restarted from scratch (first work day edit/detect, manual
    /// reset, or a lead-side weekly-hours change). A `CarryoverSyncLoaded`
    /// from a chain launched under an older gen is discarded on arrival —
    /// otherwise it can reinsert a value a newer chain already purged, and
    /// `first_missing_carryover_year` would never revisit that year.
    pub carryover_sync_gen: HashMap<i64, u64>,

    /// Company user directory for the "add member" roster picker.
    /// Populated from `UserDirectoryCache` (or a live fetch) on first use of
    /// the Team page — empty and unused for non-admin accounts.
    pub directory: Vec<crate::harvest::models::User>,
    pub directory_loading: bool,
    pub directory_error: Option<String>,
    /// Search text typed into the roster picker; also doubles as the
    /// "picker is open" signal (non-empty ⇒ suggestions are shown).
    pub directory_query: String,
}

impl TeamPageState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ── Messages ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TeamMsg {
    /// Re-fetch current-year stats for every roster member.
    Refresh,
    MemberStatsLoaded(u64, i64, Result<(YearBalance, HolidayStats), String>),

    NameChanged(String),
    HarvestIdChanged(String),
    AddMember,
    RemoveMember(i64),
    /// Start read-only impersonation of a roster member. Navigates to the
    /// Day page and bumps `entries_gen`/`stats_gen`/`vacation_gen` so any
    /// in-flight fetch for the team lead's own identity is discarded when
    /// it resolves. Deliberately does not bump `assignments_gen`,
    /// `billable_gen`, or `project_tracking_gen` — see Global Constraints.
    ImpersonationStart(i64),
    /// Exit impersonation and return to the team lead's own data. Does not
    /// force navigation — the lead stays on whatever page they were
    /// viewing. Bumps the same three generation counters as `ImpersonationStart`.
    ImpersonationExit,
    FirstWorkDayInputChanged(i64, String),
    FirstWorkDaySave(i64),
    /// Auto-detect result for a newly added member's `first_work_day`,
    /// dispatched automatically right after `AddMember` — no manual button.
    FirstWorkDayDetected(i64, Result<Option<NaiveDate>, String>),
    WorkPercentageInputChanged(i64, String),
    WorkPercentageSave(i64),

    /// Load the company user directory if it isn't already loaded this
    /// session, preferring a fresh disk cache over a live fetch. No-op for
    /// non-admin accounts. Dispatched whenever the Team page loads.
    DirectoryEnsureLoaded,
    /// Force a live re-fetch, bypassing the cache ("↺ Refresh directory").
    DirectoryRefresh,
    DirectoryLoaded(Result<Vec<crate::harvest::models::User>, String>),
    DirectoryQueryChanged(String),
    /// User picked a directory entry — fills the add-member name/ID fields.
    DirectoryPick(i64),

    CarryoverDelete(i64, i32),
    CarryoverSyncStart(i64),
    /// Carries the `carryover_sync_gen` for this member the chain was
    /// launched with — see `TeamPageState::carryover_sync_gen`.
    CarryoverSyncLoaded(i64, u64, i32, Result<(YearBalance, HolidayStats), String>),
    CarryoverReset(i64),

    AdjShowForm(i64),
    AdjHideForm(i64),
    AdjDateChanged(i64, String),
    AdjHoursChanged(i64, String),
    AdjReasonChanged(i64, String),
    AdjSubmit(i64),
    AdjDelete(i64, u64),
}

impl EasyHarvest {
    /// Returns `true` on success, so mutating handlers that need to roll
    /// back an in-memory change on failure (rather than leave displayed data
    /// diverged from disk) can check it.
    fn save_team_or_warn(&mut self) -> bool {
        match self.team_settings.save(&self.settings.data_dir) {
            Ok(()) => true,
            Err(e) => {
                self.error_banner = Some(format!("Failed to save team settings: {e}"));
                false
            }
        }
    }

    /// Kick off carryover sync for every team member, mirroring the personal
    /// account's `SettingsMsg::CarryoverSyncStart` call in `CurrentUserLoaded`.
    /// No-ops entirely when Team Lead Mode is off so non-team-lead users never
    /// pay for these extra fetches.
    pub(super) fn start_all_team_carryover_syncs(&mut self) -> Task<Message> {
        if !self.settings.team_lead_mode {
            return Task::none();
        }
        let ids: Vec<i64> = self.team_settings.members.iter().map(|m| m.harvest_user_id).collect();
        Task::batch(
            ids.into_iter().map(|id| self.update_team(TeamMsg::CarryoverSyncStart(id))).collect::<Vec<_>>()
        )
    }

    /// Every member's stored carryover was computed via
    /// `member.expected_hours_per_day(self.settings.total_weekly_hours)` —
    /// the *lead's* weekly-hours baseline, not the member's own. When the
    /// lead corrects their own weekly hours, every member's cached
    /// auto-computed carryover silently becomes wrong and would otherwise
    /// never be revisited (the gap-fill sync only ever fills in *missing*
    /// years). Purge and re-sync the whole roster, mirroring
    /// `SettingsMsg::CarryoverReset` for the personal path.
    pub(super) fn reset_all_team_carryovers(&mut self) -> Task<Message> {
        if !self.settings.team_lead_mode {
            return Task::none();
        }
        let ids: Vec<i64> = self.team_settings.members.iter().map(|m| m.harvest_user_id).collect();
        for &id in &ids {
            if let Some(member) = self.team_settings.member_mut(id) {
                member.carryover.retain(|_, v| v.is_user_defined);
                if let Some(fwd) = member.first_work_day {
                    member.carryover.entry(fwd.year()).or_default();
                }
            }
            *self.team.carryover_sync_gen.entry(id).or_default() += 1;
        }
        self.save_team_or_warn();
        Task::batch(
            ids.into_iter().map(|id| self.update_team(TeamMsg::CarryoverSyncStart(id))).collect::<Vec<_>>()
        )
    }

    pub(super) fn update_team(&mut self, msg: TeamMsg) -> Task<Message> {
        match msg {
            TeamMsg::Refresh => {
                if self.client.is_none() {
                    return Task::none();
                }
                self.team.r#gen += 1;
                for member in &self.team_settings.members {
                    self.team.stats.entry(member.harvest_user_id).or_default().loading = true;
                }
                let members: Vec<TeamMember> = self.team_settings.members.clone();
                Task::batch(
                    members.iter().map(|m| self.load_team_member_stats_task(m)).collect::<Vec<_>>()
                )
            }

            TeamMsg::MemberStatsLoaded(r#gen, user_id, result) => {
                if r#gen != self.team.r#gen { return Task::none(); }
                let entry = self.team.stats.entry(user_id).or_default();
                entry.loading = false;
                match result {
                    Ok((balance, holidays)) => {
                        entry.year_balance = Some(balance);
                        entry.holiday_stats = Some(holidays);
                        entry.error = None;
                    }
                    Err(e) => entry.error = Some(e),
                }
                Task::none()
            }

            TeamMsg::NameChanged(v) => {
                self.team.add_form.name_input = v;
                self.team.add_form.error = None;
                Task::none()
            }

            TeamMsg::HarvestIdChanged(v) => {
                self.team.add_form.id_input = v;
                self.team.add_form.error = None;
                Task::none()
            }

            TeamMsg::AddMember => {
                let name = self.team.add_form.name_input.trim().to_string();
                if name.is_empty() {
                    self.team.add_form.error = Some("Enter a name.".into());
                    return Task::none();
                }
                let id_str = self.team.add_form.id_input.trim().to_string();
                let harvest_user_id: i64 = match id_str.parse() {
                    Ok(v) if v > 0 => v,
                    _ => {
                        self.team.add_form.error =
                            Some("Enter a valid Harvest user ID (positive number).".into());
                        return Task::none();
                    }
                };
                if self.team_settings.member(harvest_user_id).is_some() {
                    self.team.add_form.error =
                        Some("A team member with this Harvest user ID already exists.".into());
                    return Task::none();
                }
                self.team_settings.members.push(TeamMember {
                    harvest_user_id,
                    display_name: name,
                    work_percentage: 1.0,
                    total_holiday_days_per_year: self.settings.total_holiday_days_per_year,
                    holiday_task_ids: self.settings.holiday_task_ids.clone(),
                    first_work_day: None,
                    carryover: HashMap::new(),
                    overtime_adjustments: OvertimeAdjustmentStore::default(),
                });
                if let Err(e) = self.team_settings.save(&self.settings.data_dir) {
                    self.team_settings.members.retain(|m| m.harvest_user_id != harvest_user_id);
                    self.error_banner = Some(format!("Failed to save team roster: {e}"));
                    return Task::none();
                }
                self.team.add_form = TeamMemberForm::default();
                Task::batch(vec![
                    Task::done(Message::Team(TeamMsg::Refresh)),
                    self.detect_member_first_work_day_task(harvest_user_id),
                ])
            }

            TeamMsg::RemoveMember(id) => {
                self.team_settings.members.retain(|m| m.harvest_user_id != id);
                self.team.stats.remove(&id);
                self.team.adjustment_forms.remove(&id);
                self.team.first_work_day_inputs.remove(&id);
                self.team.work_percentage_inputs.remove(&id);
                self.team.carryover_sync_gen.remove(&id);
                self.team.r#gen += 1;
                // Delegate to the real exit path: clearing the flag alone would
                // leave the removed member's cached entries, vacation and stats
                // on screen with no read-only banner and every mutation control
                // re-enabled. `ImpersonationExit` is keyed purely off
                // `self.impersonating` and never reads the roster, so running it
                // here — after the member is gone, before the save — is safe. It
                // must run *before* `save_team_or_warn`, because it clears
                // `error_banner` and would otherwise swallow a save failure.
                let exit = (self.impersonating == Some(id))
                    .then(|| self.update_team(TeamMsg::ImpersonationExit));
                self.save_team_or_warn();
                exit.unwrap_or_else(Task::none)
            }

            TeamMsg::ImpersonationStart(id) => {
                self.impersonating = Some(id);
                self.page = Page::Day;
                self.date_picker.open = false;
                self.entry_form = None;
                self.pending_delete = None;
                self.overtime_adj_form = None;
                self.vacation.form = None;
                self.error_banner = None;
                self.entries.clear();
                self.year_balance = None;
                self.holiday_stats = None;
                self.month_summaries = None;
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                self.entries_gen += 1;
                self.stats_gen += 1;
                self.vacation_gen += 1;
                self.loading = true;
                Task::batch([
                    self.load_entries_task(),
                    self.load_stats_task(),
                    self.load_vacation_task(),
                ])
            }

            TeamMsg::ImpersonationExit => {
                self.impersonating = None;
                self.entry_form = None;
                self.pending_delete = None;
                self.overtime_adj_form = None;
                self.vacation.form = None;
                self.error_banner = None;
                self.entries.clear();
                self.year_balance = None;
                self.holiday_stats = None;
                self.month_summaries = None;
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                self.entries_gen += 1;
                self.stats_gen += 1;
                self.vacation_gen += 1;
                self.loading = true;
                Task::batch([
                    self.load_entries_task(),
                    self.load_stats_task(),
                    self.load_vacation_task(),
                ])
            }

            TeamMsg::FirstWorkDayInputChanged(id, v) => {
                self.team.first_work_day_inputs.insert(id, v);
                Task::none()
            }

            TeamMsg::FirstWorkDaySave(id) => {
                let raw = self.team.first_work_day_inputs.get(&id).cloned().unwrap_or_default();
                let raw = raw.trim().to_string();
                let parsed = if raw.is_empty() {
                    None
                } else {
                    match NaiveDate::parse_from_str(&raw, "%d.%m.%Y") {
                        Ok(d) => Some(d),
                        Err(_) => {
                            self.error_banner =
                                Some("Invalid first work day — use DD.MM.YYYY.".into());
                            return Task::none();
                        }
                    }
                };
                let Some(member) = self.team_settings.member_mut(id) else { return Task::none(); };
                let fwd_changed = member.first_work_day != parsed;
                member.first_work_day = parsed;
                if fwd_changed {
                    // Mirrors the personal path's CarryoverReset-on-change: a
                    // corrected first_work_day invalidates any auto-computed
                    // carryover entries from the old date, which would otherwise
                    // sit as stale entries that block first_missing_carryover_year
                    // from ever resyncing those years.
                    member.carryover.retain(|_, v| v.is_user_defined);
                    *self.team.carryover_sync_gen.entry(id).or_default() += 1;
                }
                if let Some(fwd) = parsed {
                    member.carryover.entry(fwd.year()).or_default();
                }
                self.save_team_or_warn();
                self.team.first_work_day_inputs.remove(&id);
                self.update_team(TeamMsg::CarryoverSyncStart(id))
            }

            TeamMsg::FirstWorkDayDetected(id, result) => {
                match result {
                    Ok(Some(date)) => {
                        let Some(member) = self.team_settings.member_mut(id) else {
                            return Task::none();
                        };
                        if member.first_work_day != Some(date) {
                            member.carryover.retain(|_, c| c.is_user_defined);
                            *self.team.carryover_sync_gen.entry(id).or_default() += 1;
                        }
                        member.first_work_day = Some(date);
                        member.carryover.entry(date.year()).or_default();
                        self.save_team_or_warn();
                        self.update_team(TeamMsg::CarryoverSyncStart(id))
                    }
                    // No entries yet — expected for a brand-new hire, not an error.
                    Ok(None) => Task::none(),
                    Err(e) => {
                        self.error_banner =
                            Some(format!("Auto-detect first work day failed: {e}"));
                        Task::none()
                    }
                }
            }

            TeamMsg::WorkPercentageInputChanged(id, v) => {
                self.team.work_percentage_inputs.insert(id, v);
                Task::none()
            }

            TeamMsg::WorkPercentageSave(id) => {
                let raw = self.team.work_percentage_inputs.get(&id).cloned().unwrap_or_default();
                let percentage = match raw.trim().replace(',', ".").parse::<f64>() {
                    Ok(v) if v > 0.0 && v <= 100.0 => v / 100.0,
                    _ => {
                        self.error_banner = Some("Invalid work percentage (1–100).".into());
                        return Task::none();
                    }
                };
                let Some(member) = self.team_settings.member_mut(id) else { return Task::none(); };
                member.work_percentage = percentage;
                self.save_team_or_warn();
                self.team.work_percentage_inputs.remove(&id);
                Task::none()
            }

            TeamMsg::CarryoverDelete(id, year) => {
                if let Some(member) = self.team_settings.member_mut(id) {
                    member.carryover.remove(&year);
                }
                self.save_team_or_warn();
                Task::none()
            }

            TeamMsg::CarryoverReset(id) => {
                if let Some(member) = self.team_settings.member_mut(id) {
                    member.carryover.retain(|_, v| v.is_user_defined);
                    if let Some(fwd) = member.first_work_day {
                        member.carryover.entry(fwd.year()).or_default();
                    }
                }
                self.save_team_or_warn();
                *self.team.carryover_sync_gen.entry(id).or_default() += 1;
                self.update_team(TeamMsg::CarryoverSyncStart(id))
            }

            TeamMsg::CarryoverSyncStart(id) => {
                let current_year = Local::now().naive_local().date().year();
                let Some(member) = self.team_settings.member(id) else { return Task::none(); };
                let Some(fwd) = member.first_work_day else { return Task::none(); };
                let start = fwd.year();
                let r#gen = *self.team.carryover_sync_gen.entry(id).or_default();
                match crate::stats::first_missing_carryover_year(&member.carryover, start, current_year) {
                    Some(first_missing) => self.load_team_carryover_sync_task(member, first_missing, r#gen),
                    None => Task::none(),
                }
            }

            TeamMsg::CarryoverSyncLoaded(id, r#gen, year, result) => {
                if self.team.carryover_sync_gen.get(&id).copied().unwrap_or(0) != r#gen {
                    // Superseded by a newer reset for this member — discard.
                    return Task::none();
                }
                let next = year + 1;
                let lead_weekly_hours = self.settings.total_weekly_hours;
                if let Some(member) = self.team_settings.member_mut(id) {
                    match result {
                        Ok((balance, holidays)) => {
                            let user_defined =
                                member.carryover.get(&next).is_some_and(|c| c.is_user_defined);
                            if !user_defined {
                                let epd = member.expected_hours_per_day(lead_weekly_hours);
                                member.carryover.insert(next, YearCarryover {
                                    overtime_hours: balance.total_balance,
                                    holiday_hours: holidays.days_remaining * epd,
                                    ..Default::default()
                                });
                            }
                        }
                        Err(_) => {
                            member.carryover.entry(next).or_default();
                        }
                    }
                }
                self.save_team_or_warn();
                let sync_task = self.update_team(TeamMsg::CarryoverSyncStart(id));
                // If that was the last missing year, the member's displayed
                // stats (computed earlier against incomplete carryover, since
                // this chain runs sequentially over potentially many years)
                // are now stale — refresh them. Without this, a card can show
                // wrong overtime/vacation numbers until the next manual
                // Refresh or Team page revisit.
                let current_year = Local::now().naive_local().date().year();
                let chain_finished = self.team_settings.member(id).is_some_and(|m| {
                    m.first_work_day.is_some_and(|fwd| {
                        crate::stats::first_missing_carryover_year(&m.carryover, fwd.year(), current_year)
                            .is_none()
                    })
                });
                if chain_finished {
                    let stats_task = match self.team_settings.member(id) {
                        Some(member) => self.load_team_member_stats_task(member),
                        None => Task::none(),
                    };
                    Task::batch([sync_task, stats_task])
                } else {
                    sync_task
                }
            }

            TeamMsg::AdjShowForm(id) => {
                self.team.adjustment_forms.insert(id, OvertimeAdjustmentForm::default());
                Task::none()
            }

            TeamMsg::AdjHideForm(id) => {
                self.team.adjustment_forms.remove(&id);
                Task::none()
            }

            TeamMsg::AdjDateChanged(id, v) => {
                if let Some(f) = self.team.adjustment_forms.get_mut(&id) { f.date_input = v; f.error = None; }
                Task::none()
            }

            TeamMsg::AdjHoursChanged(id, v) => {
                if let Some(f) = self.team.adjustment_forms.get_mut(&id) { f.hours_input = v; f.error = None; }
                Task::none()
            }

            TeamMsg::AdjReasonChanged(id, v) => {
                if let Some(f) = self.team.adjustment_forms.get_mut(&id) { f.reason_input = v; f.error = None; }
                Task::none()
            }

            TeamMsg::AdjSubmit(id) => {
                let current_year = Local::now().naive_local().date().year();
                let Some(form) = self.team.adjustment_forms.get(&id).cloned() else { return Task::none(); };
                let validated = match form.validate(current_year) {
                    Ok(v) => v,
                    Err(e) => {
                        if let Some(f) = self.team.adjustment_forms.get_mut(&id) { f.error = Some(e); }
                        return Task::none();
                    }
                };
                let adj_year = validated.date.year();
                if let Some(member) = self.team_settings.member_mut(id) {
                    let adj_id = member.overtime_adjustments.next_id;
                    member.overtime_adjustments.next_id += 1;
                    member.overtime_adjustments.adjustments_for_mut(adj_year).push(
                        crate::state::overtime_adjustments::OvertimeAdjustment {
                            id: adj_id,
                            date: validated.date.format("%Y-%m-%d").to_string(),
                            hours: validated.hours,
                            reason: validated.reason,
                        }
                    );
                    if !self.save_team_or_warn() {
                        // Roll back so displayed data matches disk, and leave the
                        // form open so the lead can retry instead of it silently
                        // closing on a failed save.
                        if let Some(member) = self.team_settings.member_mut(id) {
                            member.overtime_adjustments.adjustments_for_mut(adj_year).retain(|a| a.id != adj_id);
                        }
                        return Task::none();
                    }
                }
                self.team.adjustment_forms.remove(&id);
                Task::done(Message::Team(TeamMsg::Refresh))
            }

            TeamMsg::AdjDelete(id, adj_id) => {
                let current_year = Local::now().naive_local().date().year();
                let removed: Vec<_> = self.team_settings.member(id)
                    .map(|m| {
                        m.overtime_adjustments.adjustments_for(current_year)
                            .iter()
                            .filter(|a| a.id == adj_id)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(member) = self.team_settings.member_mut(id) {
                    member.overtime_adjustments.adjustments_for_mut(current_year).retain(|a| a.id != adj_id);
                }
                if !self.save_team_or_warn() {
                    // Roll back so displayed data matches disk.
                    if let Some(member) = self.team_settings.member_mut(id) {
                        member.overtime_adjustments.adjustments_for_mut(current_year).extend(removed);
                    }
                    return Task::none();
                }
                Task::done(Message::Team(TeamMsg::Refresh))
            }

            TeamMsg::DirectoryEnsureLoaded => {
                if !self.harvest_user_is_admin {
                    return Task::none();
                }
                if !self.team.directory.is_empty() {
                    return Task::none();
                }
                if let Some(cache) = crate::state::cache::UserDirectoryCache::load(&self.settings.data_dir)
                    && cache.is_valid()
                {
                    self.team.directory = cache.users;
                    return Task::none();
                }
                if self.client.is_none() {
                    return Task::none();
                }
                self.team.directory_loading = true;
                self.team.directory_error = None;
                self.load_team_directory_task()
            }

            TeamMsg::DirectoryRefresh => {
                if !self.harvest_user_is_admin || self.client.is_none() {
                    return Task::none();
                }
                self.team.directory_loading = true;
                self.team.directory_error = None;
                self.load_team_directory_task()
            }

            TeamMsg::DirectoryLoaded(result) => {
                self.team.directory_loading = false;
                match result {
                    Ok(users) => {
                        self.team.directory_error = None;
                        let cache = crate::state::cache::UserDirectoryCache::new(users.clone());
                        if let Err(e) = cache.save(&self.settings.data_dir) {
                            self.error_banner = Some(format!("Failed to cache user directory: {e}"));
                        }
                        self.team.directory = users;
                    }
                    Err(e) => self.team.directory_error = Some(e),
                }
                Task::none()
            }

            TeamMsg::DirectoryQueryChanged(v) => {
                self.team.directory_query = v;
                Task::none()
            }

            TeamMsg::DirectoryPick(id) => {
                if let Some(user) = self.team.directory.iter().find(|u| u.id == id) {
                    self.team.add_form.name_input = format!("{} {}", user.first_name, user.last_name);
                    self.team.add_form.id_input = user.id.to_string();
                    self.team.add_form.error = None;
                }
                self.team.directory_query = String::new();
                Task::none()
            }
        }
    }
}
