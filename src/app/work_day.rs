use super::*;

// ── Work Day edit state ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct WorkDayEditState {
    pub edit_mode: bool,
    pub start_input: String,
    pub end_input: String,
    /// (break_start_str, break_end_str) per break in edit mode
    pub break_inputs: Vec<(String, String)>,
}

// ── Work Day messages ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum WorkDayMsg {
    Start,
    StartBreak,
    EndBreak,
    End,
    Resume,
    Tick,
    EditStart,
    EditCancel,
    StartInputChanged(String),
    EndInputChanged(String),
    BreakStartChanged(usize, String),
    BreakEndChanged(usize, String),
    BreakDelete(usize),
    BreakAdd,
    EditSave,
}

impl EasyHarvest {
    /// Save the work day store and surface any error via the error banner.
    fn save_work_day(&mut self) {
        if let Err(e) = self.work_day_store.save(&self.settings.data_dir) {
            self.error_banner = Some(format!("Failed to save work day: {e}"));
        }
    }

    pub(super) fn update_work_day(&mut self, msg: WorkDayMsg) -> Task<Message> {
        match msg {
            WorkDayMsg::Start => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.start(now.time());
                self.work_day_store.set(day);
                self.save_work_day();
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }

            WorkDayMsg::StartBreak => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.start_break(now.time());
                self.work_day_store.set(day);
                self.save_work_day();
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }

            WorkDayMsg::EndBreak => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.end_break(now.time());
                self.work_day_store.set(day);
                self.save_work_day();
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }

            WorkDayMsg::End => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.end(now.time());
                self.work_day_store.set(day);
                self.save_work_day();
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }

            WorkDayMsg::Resume => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                // Record the "off" gap as a break so worked hours stay accurate.
                if let Some(ended_at) = day.end_time {
                    day.breaks.push(crate::state::work_day::Break {
                        start: ended_at,
                        end: Some(now.time()),
                    });
                }
                day.end_time = None;
                self.work_day_store.set(day);
                self.save_work_day();
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }

            WorkDayMsg::Tick => {
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }

            WorkDayMsg::EditStart => {
                let day = self.work_day_store.get_or_default(self.current_date);
                self.work_day_edit.start_input = day.start_time
                    .map(|t| t.format("%H:%M").to_string())
                    .unwrap_or_default();
                self.work_day_edit.end_input = day.end_time
                    .map(|t| t.format("%H:%M").to_string())
                    .unwrap_or_default();
                self.work_day_edit.break_inputs = day.breaks.iter().map(|b| (
                    b.start.format("%H:%M").to_string(),
                    b.end.map(|t| t.format("%H:%M").to_string()).unwrap_or_default(),
                )).collect();
                self.work_day_edit.edit_mode = true;
                Task::none()
            }

            WorkDayMsg::EditCancel => {
                self.work_day_edit.edit_mode = false;
                self.work_day_edit.break_inputs.clear();
                Task::none()
            }

            WorkDayMsg::StartInputChanged(v) => {
                self.work_day_edit.start_input = v;
                Task::none()
            }

            WorkDayMsg::EndInputChanged(v) => {
                self.work_day_edit.end_input = v;
                Task::none()
            }

            WorkDayMsg::BreakStartChanged(idx, v) => {
                if let Some(b) = self.work_day_edit.break_inputs.get_mut(idx) {
                    b.0 = v;
                }
                Task::none()
            }

            WorkDayMsg::BreakEndChanged(idx, v) => {
                if let Some(b) = self.work_day_edit.break_inputs.get_mut(idx) {
                    b.1 = v;
                }
                Task::none()
            }

            WorkDayMsg::BreakDelete(idx) => {
                if idx < self.work_day_edit.break_inputs.len() {
                    self.work_day_edit.break_inputs.remove(idx);
                }
                Task::none()
            }

            WorkDayMsg::BreakAdd => {
                self.work_day_edit.break_inputs.push((String::new(), String::new()));
                Task::none()
            }

            WorkDayMsg::EditSave => {
                use chrono::NaiveTime;
                let mut day = self.work_day_store.get_or_default(self.current_date);
                let mut errors: Vec<&str> = Vec::new();

                if !self.work_day_edit.start_input.is_empty() {
                    match NaiveTime::parse_from_str(&self.work_day_edit.start_input, "%H:%M") {
                        Ok(t) => day.start_time = Some(t),
                        Err(_) => errors.push("Invalid start time"),
                    }
                }

                if !self.work_day_edit.end_input.is_empty() {
                    match NaiveTime::parse_from_str(&self.work_day_edit.end_input, "%H:%M") {
                        Ok(t) => day.end_time = Some(t),
                        Err(_) => errors.push("Invalid end time"),
                    }
                } else {
                    day.end_time = None;
                }

                if !errors.is_empty() {
                    self.error_banner = Some(errors.join(", ") + " (use HH:MM)");
                    return Task::none();
                }

                // Validate envelope: end must be after start when both are set.
                if let (Some(s), Some(e)) = (day.start_time, day.end_time) {
                    if e <= s {
                        self.error_banner = Some("End time must be after start time".into());
                        return Task::none();
                    }
                }

                let mut break_errors = false;
                let breaks: Vec<_> = self.work_day_edit.break_inputs.iter().filter_map(|(s, e)| {
                    let start = match NaiveTime::parse_from_str(s, "%H:%M") {
                        Ok(t) => t,
                        Err(_) => { break_errors = true; return None; }
                    };
                    let end = if e.is_empty() {
                        None
                    } else {
                        match NaiveTime::parse_from_str(e, "%H:%M") {
                            Ok(t) => Some(t),
                            Err(_) => { break_errors = true; return None; }
                        }
                    };
                    if let Some(nd) = end {
                        if start >= nd { break_errors = true; return None; }
                    }
                    Some(crate::state::work_day::Break { start, end })
                }).collect();

                if break_errors {
                    self.error_banner = Some("Invalid break times (use HH:MM, start < end)".into());
                    return Task::none();
                }

                day.breaks = breaks;

                self.work_day_store.set(day);
                self.save_work_day();
                self.work_day_edit.edit_mode = false;
                self.work_day_edit.break_inputs.clear();
                #[cfg(not(target_os = "macos"))]
                self.sync_tray_phase();
                Task::none()
            }
        }
    }
}
