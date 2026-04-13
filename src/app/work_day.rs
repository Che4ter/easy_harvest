use super::*;

// ── Work Day ─────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(super) fn update_work_day(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartDay => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.start(now.time());
                self.work_day_store.set(day);
                let _ = self.work_day_store.save(&self.settings.data_dir);
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            Message::StartBreak => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.start_break(now.time());
                self.work_day_store.set(day);
                let _ = self.work_day_store.save(&self.settings.data_dir);
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            Message::EndBreak => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.end_break(now.time());
                self.work_day_store.set(day);
                let _ = self.work_day_store.save(&self.settings.data_dir);
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            Message::EndDay => {
                let now = Local::now().naive_local();
                let mut day = self.work_day_store.get_or_default(now.date());
                day.end(now.time());
                self.work_day_store.set(day);
                let _ = self.work_day_store.save(&self.settings.data_dir);
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            Message::ResumeDay => {
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
                let _ = self.work_day_store.save(&self.settings.data_dir);
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            Message::WorkDayTick => {
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            Message::WorkDayEditStart => {
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

            Message::WorkDayEditCancel => {
                self.work_day_edit.edit_mode = false;
                self.work_day_edit.break_inputs.clear();
                Task::none()
            }

            Message::WorkDayStartInputChanged(v) => {
                self.work_day_edit.start_input = v;
                Task::none()
            }

            Message::WorkDayEndInputChanged(v) => {
                self.work_day_edit.end_input = v;
                Task::none()
            }

            Message::WorkDayBreakStartChanged(idx, v) => {
                if let Some(b) = self.work_day_edit.break_inputs.get_mut(idx) {
                    b.0 = v;
                }
                Task::none()
            }

            Message::WorkDayBreakEndChanged(idx, v) => {
                if let Some(b) = self.work_day_edit.break_inputs.get_mut(idx) {
                    b.1 = v;
                }
                Task::none()
            }

            Message::WorkDayBreakDelete(idx) => {
                if idx < self.work_day_edit.break_inputs.len() {
                    self.work_day_edit.break_inputs.remove(idx);
                }
                Task::none()
            }

            Message::WorkDayBreakAdd => {
                self.work_day_edit.break_inputs.push((String::new(), String::new()));
                Task::none()
            }

            Message::WorkDayEditSave => {
                use chrono::NaiveTime;
                let mut day = self.work_day_store.get_or_default(self.current_date);

                if !self.work_day_edit.start_input.is_empty() {
                    if let Ok(t) = NaiveTime::parse_from_str(&self.work_day_edit.start_input, "%H:%M") {
                        day.start_time = Some(t);
                    }
                }

                day.end_time = if self.work_day_edit.end_input.is_empty() {
                    None
                } else {
                    NaiveTime::parse_from_str(&self.work_day_edit.end_input, "%H:%M").ok()
                };

                let breaks: Vec<_> = self.work_day_edit.break_inputs.iter().filter_map(|(s, e)| {
                    let start = NaiveTime::parse_from_str(s, "%H:%M").ok()?;
                    let end = if e.is_empty() {
                        None
                    } else {
                        NaiveTime::parse_from_str(e, "%H:%M").ok()
                    };
                    if let Some(nd) = end {
                        if start >= nd { return None; }
                    }
                    Some(crate::state::work_day::Break { start, end })
                }).collect();

                day.breaks = breaks;

                self.work_day_store.set(day);
                let _ = self.work_day_store.save(&self.settings.data_dir);
                self.work_day_edit.edit_mode = false;
                self.work_day_edit.break_inputs.clear();
                #[cfg(target_os = "linux")]
                self.sync_tray_phase();
                Task::none()
            }

            _ => unreachable!(),
        }
    }
}
