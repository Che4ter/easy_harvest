use super::*;
use super::tasks::build_vacation_entries;

// ── Vacation ─────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(super) fn update_vacation(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::VacationYearPrev => {
                self.vacation.year -= 1;
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.vacation_gen += 1;
                    self.load_vacation_task()
                } else {
                    Task::none()
                }
            }

            Message::VacationYearNext => {
                self.vacation.year += 1;
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.vacation_gen += 1;
                    self.load_vacation_task()
                } else {
                    Task::none()
                }
            }

            Message::VacationRefresh => {
                self.vacation.entries.clear();
                self.vacation.entries.shrink_to_fit();
                self.vacation.summary = None;
                if self.client.is_some() {
                    self.loading = true;
                    self.vacation_gen += 1;
                    self.load_vacation_task()
                } else {
                    Task::none()
                }
            }

            Message::VacationEntriesLoaded(gen, result) => {
                if gen != self.vacation_gen { return Task::none(); }
                self.loading = false;
                match result {
                    Ok(entries) => {
                        self.vacation.entries = entries;
                        self.recompute_vacation_summary();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            Message::VacationShowForm => {
                self.vacation.form = Some(VacationForm::new());
                Task::none()
            }

            Message::VacationHideForm => {
                self.vacation.form = None;
                Task::none()
            }

            Message::VacationFromChanged(v) => {
                if let Some(f) = &mut self.vacation.form {
                    f.from_input = v;
                    f.error = None;
                }
                Task::none()
            }

            Message::VacationToChanged(v) => {
                if let Some(f) = &mut self.vacation.form {
                    f.to_input = v;
                    f.error = None;
                }
                Task::none()
            }

            Message::VacationDayTypeFull => {
                if let Some(f) = &mut self.vacation.form { f.full_day = true; }
                Task::none()
            }

            Message::VacationDayTypeHalf => {
                if let Some(f) = &mut self.vacation.form { f.full_day = false; }
                Task::none()
            }

            Message::VacationFormSubmit => {
                let Some(form) = &mut self.vacation.form else { return Task::none(); };
                let parse = |s: &str| {
                    NaiveDate::parse_from_str(s.trim(), "%d.%m.%Y")
                        .or_else(|_| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d"))
                        .ok()
                };
                let from = parse(&form.from_input.clone());
                let to_str = if form.to_input.trim().is_empty() {
                    form.from_input.clone()
                } else {
                    form.to_input.clone()
                };
                let to = parse(&to_str);
                match (from, to) {
                    (Some(from), Some(to)) => {
                        let full_day = form.full_day;
                        let hours_full = self.settings.expected_hours_per_day();
                        let hours = if full_day { hours_full } else { hours_full / 2.0 };
                        // find project_id + task_id from assignments
                        let task_id = self.settings.holiday_task_ids.first().copied();
                        let Some(task_id) = task_id else {
                            form.error = Some("No holiday task configured in Settings.".into());
                            return Task::none();
                        };
                        let project_id = self.assignments.iter().find_map(|a| {
                            if a.task_assignments.iter().any(|t| t.task.id == task_id) {
                                Some(a.project.id)
                            } else {
                                None
                            }
                        });
                        let Some(project_id) = project_id else {
                            form.error = Some("Could not find project for holiday task. Try syncing assignments.".into());
                            return Task::none();
                        };
                        match build_vacation_entries(from, to, self.vacation.year, hours, project_id, task_id) {
                            Ok(entries_to_create) => {
                                form.submitting = true;
                                form.error = None;
                                self.submit_vacation_task(entries_to_create)
                            }
                            Err(e) => {
                                form.error = Some(e);
                                Task::none()
                            }
                        }
                    }
                    _ => {
                        if let Some(f) = &mut self.vacation.form {
                            f.error = Some("Enter valid dates in DD.MM.YYYY format. From must be \u{2264} To.".into());
                        }
                        Task::none()
                    }
                }
            }

            Message::VacationEntriesCreated(result) => {
                if let Some(f) = &mut self.vacation.form {
                    f.submitting = false;
                }
                match result {
                    Ok(new_entries) => {
                        self.vacation.form = None;
                        self.vacation.entries.extend(new_entries);
                        self.vacation.entries.sort_by(|a, b| a.spent_date.cmp(&b.spent_date));
                        self.recompute_vacation_summary();
                    }
                    Err(e) => {
                        if let Some(f) = &mut self.vacation.form {
                            f.error = Some(e);
                        }
                    }
                }
                Task::none()
            }

            Message::VacationDeleteEntry(id) => {
                let Some(client) = self.client.clone() else { return Task::none(); };
                Task::perform(
                    async move {
                        client.delete_time_entry(id).await
                            .map(|_| id)
                            .map_err(|e| e.to_string())
                    },
                    Message::VacationEntryDeleted,
                )
            }

            Message::VacationEntryDeleted(result) => {
                match result {
                    Ok(id) => {
                        self.vacation.entries.retain(|e| e.id != id);
                        self.recompute_vacation_summary();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            _ => unreachable!(),
        }
    }
}
