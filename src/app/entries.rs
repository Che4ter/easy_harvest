use super::*;

// ── Entry form ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EntryForm {
    /// None = create new; Some(id) = editing existing
    pub editing_id: Option<i64>,
    pub project_query: String,
    pub selected_project_idx: Option<usize>,
    /// M4-F2: stable (project_id, task_id) key that survives an assignments
    /// refresh — preferred over the index in Submit to avoid stale-index bugs.
    pub selected_project_key: Option<(i64, i64)>,
    pub hours_input: String,
    pub notes_input: String,
    pub error: Option<String>,
}

impl EntryForm {
    pub fn new() -> Self {
        Self {
            editing_id: None,
            project_query: String::new(),
            selected_project_idx: None,
            selected_project_key: None,
            hours_input: String::new(),
            notes_input: String::new(),
            error: None,
        }
    }

    pub fn for_entry(entry: &TimeEntry, options: &[crate::state::favorites::ProjectOption]) -> Self {
        let display = format!(
            "{} > {} — {}",
            entry.client.name, entry.project.name, entry.task.name
        );
        let idx = options.iter().position(|o| {
            o.project_id == entry.project.id && o.task_id == entry.task.id
        });
        let key = Some((entry.project.id, entry.task.id));
        Self {
            editing_id: Some(entry.id),
            project_query: if let Some(i) = idx {
                options[i].search_text.clone()
            } else {
                display
            },
            selected_project_idx: idx,
            selected_project_key: key,
            hours_input: crate::ui::format_hhmm(entry.hours),
            notes_input: entry.notes.clone().unwrap_or_default(),
            error: None,
        }
    }
}

impl Default for EntryForm {
    fn default() -> Self {
        Self::new()
    }
}

// ── Entries / Timer ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EntryMsg {
    Loaded(u64, Result<Vec<TimeEntry>, String>),
    AssignmentsLoaded(u64, Result<Vec<ProjectAssignment>, String>),
    SyncAssignments,
    ShowForm,
    Edit(i64),
    CancelForm,
    ProjectQueryChanged(String),
    ProjectSelected(usize),
    HoursChanged(String),
    NotesChanged(String),
    FocusHours,
    FocusNotes,
    Submit,
    Created(Result<TimeEntry, String>),
    Updated(Result<TimeEntry, String>),
    DeleteRequest(i64),
    DeleteCancel,
    Delete(i64),
    Deleted(Result<i64, String>),
    TimerStart(i64),
    TimerStop(i64),
    TimerStarted(Result<TimeEntry, String>),
    TimerStopped(Result<TimeEntry, String>),
    TemplateApply(usize),
    /// Fill the hours field with the remaining unbooked worked time.
    FillRemaining,
}

impl EasyHarvest {
    pub(super) fn update_entries(&mut self, msg: EntryMsg) -> Task<Message> {
        match msg {
            EntryMsg::Loaded(r#gen, result) => {
                if r#gen != self.entries_gen { return Task::none(); }
                self.loading = false;
                self.pending_delete = None;
                match result {
                    Ok(entries) => self.entries = entries,
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            EntryMsg::AssignmentsLoaded(r#gen, result) => {
                // M4-F3: discard stale responses that arrived after a newer load
                // was dispatched (e.g. user switched pages mid-request).
                if r#gen != self.assignments_gen { return Task::none(); }
                match result {
                    Ok(assignments) => {
                        self.assignments = assignments;
                        self.recompute_project_options();
                        self.recompute_task_list();
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            EntryMsg::SyncAssignments => {
                self.assignments_gen += 1;
                self.force_load_assignments_task()
            }

            EntryMsg::ShowForm => {
                self.entry_form = Some(EntryForm::new());
                Task::none()
            }

            EntryMsg::Edit(id) => {
                if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
                    self.entry_form = Some(EntryForm::for_entry(entry, &self.cached_project_options));
                }
                Task::none()
            }

            EntryMsg::CancelForm => {
                self.entry_form = None;
                Task::none()
            }

            EntryMsg::ProjectQueryChanged(q) => {
                if let Some(form) = &mut self.entry_form {
                    form.project_query = q;
                    form.selected_project_idx = None;
                    form.selected_project_key = None;
                }
                Task::none()
            }

            EntryMsg::ProjectSelected(idx) => {
                if let Some(form) = &mut self.entry_form {
                    let options = self.cached_project_options.clone();
                    if let Some(opt) = options.get(idx) {
                        form.project_query = opt.search_text.clone();
                        form.selected_project_idx = Some(idx);
                        // M4-F2: also store the stable key so Submit can re-resolve
                        // after an assignments refresh invalidates the index.
                        form.selected_project_key = Some((opt.project_id, opt.task_id));
                    }
                }
                Task::none()
            }

            EntryMsg::HoursChanged(h) => {
                if let Some(form) = &mut self.entry_form {
                    form.hours_input = h;
                }
                Task::none()
            }

            EntryMsg::NotesChanged(n) => {
                if let Some(form) = &mut self.entry_form {
                    form.notes_input = n;
                }
                Task::none()
            }

            EntryMsg::FocusHours => {
                iced::widget::operation::focus(iced::widget::Id::new("form_hours"))
            }

            EntryMsg::FocusNotes => {
                iced::widget::operation::focus(iced::widget::Id::new("form_notes"))
            }

            EntryMsg::Submit => {
                let Some(form) = &self.entry_form else {
                    return Task::none();
                };
                let hours: f64 = match crate::ui::parse_hours(&form.hours_input) {
                    Some(h) => h,
                    None => {
                        if let Some(f) = &mut self.entry_form {
                            f.error = Some("Enter a valid number of hours".into());
                        }
                        return Task::none();
                    }
                };

                let options = self.cached_project_options.clone();
                // M4-F2: prefer the stable (project_id, task_id) key — it survives
                // an assignments refresh that would invalidate the stored index.
                let opt = match form.selected_project_key {
                    Some((project_id, task_id)) => options
                        .iter()
                        .find(|o| o.project_id == project_id && o.task_id == task_id)
                        .cloned(),
                    None => form.selected_project_idx
                        .and_then(|idx| options.get(idx).cloned())
                        .or_else(|| {
                            options.iter().find(|o| {
                                o.search_text.to_lowercase()
                                    == form.project_query.to_lowercase()
                            }).cloned()
                        }),
                };

                let Some(opt) = opt else {
                    if let Some(f) = &mut self.entry_form {
                        f.error = Some("Select a project and task".into());
                    }
                    return Task::none();
                };

                let notes = form.notes_input.trim().to_string();
                let notes_opt = if notes.is_empty() { None } else { Some(notes) };
                let editing_id = form.editing_id;
                let date = self.current_date.format("%Y-%m-%d").to_string();
                let Some(client) = self.client.clone() else {
                    return Task::none();
                };

                // Record usage in favorites
                self.favorites.record_use(opt.project_id, opt.task_id);
                if let Err(e) = self.favorites.save(&self.settings.data_dir) {
                    self.error_banner = Some(format!("Failed to save favorites: {e}"));
                }
                self.recompute_project_options();

                if let Some(edit_id) = editing_id {
                    let update = UpdateTimeEntry {
                        project_id: Some(opt.project_id),
                        task_id: Some(opt.task_id),
                        spent_date: Some(date),
                        hours: Some(hours),
                        notes: notes_opt,
                    };
                    Task::perform(
                        async move {
                            client
                                .update_time_entry(edit_id, &update)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| Message::Entry(Box::new(EntryMsg::Updated(result))),
                    )
                } else {
                    let create = CreateTimeEntry {
                        project_id: opt.project_id,
                        task_id: opt.task_id,
                        spent_date: date,
                        hours,
                        notes: notes_opt,
                    };
                    Task::perform(
                        async move {
                            client
                                .create_time_entry(&create)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| Message::Entry(Box::new(EntryMsg::Created(result))),
                    )
                }
            }

            EntryMsg::Created(result) => {
                match result {
                    Ok(entry) => {
                        self.entries.push(entry);
                        self.entry_form = None;
                    }
                    Err(e) => {
                        if let Some(f) = &mut self.entry_form {
                            f.error = Some(e);
                        }
                    }
                }
                Task::none()
            }

            EntryMsg::Updated(result) => {
                match result {
                    Ok(updated) => {
                        if let Some(pos) =
                            self.entries.iter().position(|e| e.id == updated.id)
                        {
                            self.entries[pos] = updated;
                        }
                        self.entry_form = None;
                    }
                    Err(e) => {
                        if let Some(f) = &mut self.entry_form {
                            f.error = Some(e);
                        }
                    }
                }
                Task::none()
            }

            EntryMsg::DeleteRequest(id) => {
                self.pending_delete = Some(id);
                Task::none()
            }

            EntryMsg::DeleteCancel => {
                self.pending_delete = None;
                Task::none()
            }

            EntryMsg::Delete(id) => {
                self.pending_delete = None;
                let Some(client) = self.client.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        client
                            .delete_time_entry(id)
                            .await
                            .map(|_| id)
                            .map_err(|e| e.to_string())
                    },
                    |result| Message::Entry(Box::new(EntryMsg::Deleted(result))),
                )
            }

            EntryMsg::Deleted(result) => {
                match result {
                    Ok(id) => self.entries.retain(|e| e.id != id),
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            EntryMsg::TimerStart(id) => {
                let Some(client) = self.client.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move { client.restart_timer(id).await.map_err(|e| e.to_string()) },
                    |result| Message::Entry(Box::new(EntryMsg::TimerStarted(result))),
                )
            }

            EntryMsg::TimerStop(id) => {
                let Some(client) = self.client.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move { client.stop_timer(id).await.map_err(|e| e.to_string()) },
                    |result| Message::Entry(Box::new(EntryMsg::TimerStopped(result))),
                )
            }

            EntryMsg::TimerStarted(result) => {
                match result {
                    Ok(updated) => {
                        // M4-F5: discard responses for entries that belong to a
                        // different date — the user navigated away while the request
                        // was in flight, so these entries are no longer displayed.
                        let current = self.current_date.format("%Y-%m-%d").to_string();
                        if updated.spent_date != current {
                            return Task::none();
                        }
                        // M4-F1: Harvest allows only one running timer at a time;
                        // clear is_running on all other entries so the UI stays
                        // consistent with the server state.
                        for e in &mut self.entries {
                            if e.id == updated.id {
                                *e = updated.clone();
                            } else {
                                e.is_running = false;
                            }
                        }
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            EntryMsg::TimerStopped(result) => {
                match result {
                    Ok(updated) => {
                        // M4-F5: discard stale responses for a different date.
                        let current = self.current_date.format("%Y-%m-%d").to_string();
                        if updated.spent_date != current {
                            return Task::none();
                        }
                        if let Some(e) = self.entries.iter_mut().find(|e| e.id == updated.id) {
                            *e = updated;
                        }
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            EntryMsg::FillRemaining => {
                if self.entry_form.is_none() {
                    return Task::none();
                }
                let booked: f64 = self.entries.iter().map(|e| e.hours).sum();
                let worked_h = self
                    .work_day_store
                    .get_or_default(self.current_date)
                    .worked_hours(chrono::Local::now().naive_local().time());
                let target = worked_h.max(self.settings.expected_hours_per_day());
                // If editing an existing entry, exclude its hours from booked so
                // the fill amount is the true gap, not double-counting.
                let editing_hours = self
                    .entry_form
                    .as_ref()
                    .and_then(|f| f.editing_id)
                    .and_then(|id| self.entries.iter().find(|e| e.id == id))
                    .map(|e| e.hours)
                    .unwrap_or(0.0);
                let remaining = (target - (booked - editing_hours)).max(0.0);
                let total_mins = (remaining * 60.0).round() as u32;
                let h = total_mins / 60;
                let m = total_mins % 60;
                if let Some(form) = &mut self.entry_form {
                    form.hours_input = format!("{h}:{m:02}");
                }
                Task::none()
            }

            EntryMsg::TemplateApply(idx) => {
                let Some(tpl) = self.templates.entries.get(idx).cloned() else {
                    return Task::none();
                };
                let opts = self.cached_project_options.clone();
                let pos = opts.iter().position(|o| {
                    o.project_id == tpl.project_id && o.task_id == tpl.task_id
                });
                let form = self.entry_form.get_or_insert_with(EntryForm::new);
                if let Some(p) = pos {
                    let opt = &opts[p];
                    form.project_query =
                        format!("{} \u{203a} {} \u{2014} {}", opt.client_name, opt.project_name, opt.task_name);
                    form.selected_project_idx = Some(p);
                    // M4-F2: store stable key alongside the index.
                    form.selected_project_key = Some((opt.project_id, opt.task_id));
                }
                form.hours_input = tpl.hours.clone();
                form.notes_input = tpl.notes.clone();
                Task::none()
            }
        }
    }
}
