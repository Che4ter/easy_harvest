use super::*;

// ── Entries / Timer ──────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(super) fn update_entries(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EntriesLoaded(gen, result) => {
                if gen != self.entries_gen { return Task::none(); }
                self.loading = false;
                self.pending_delete = None;
                match result {
                    Ok(entries) => self.entries = entries,
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            Message::AssignmentsLoaded(result) => {
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

            Message::SyncAssignments => self.load_assignments_task(),

            Message::StatsLoaded(gen, result) => {
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

            Message::ShowAddForm => {
                self.entry_form = Some(EntryForm::new());
                Task::none()
            }

            Message::EditEntry(id) => {
                if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
                    self.entry_form = Some(EntryForm::for_entry(entry));
                }
                Task::none()
            }

            Message::CancelForm => {
                self.entry_form = None;
                Task::none()
            }

            Message::FormProjectQueryChanged(q) => {
                if let Some(form) = &mut self.entry_form {
                    form.project_query = q;
                    form.selected_project_idx = None;
                }
                Task::none()
            }

            Message::FormProjectSelected(idx) => {
                if let Some(form) = &mut self.entry_form {
                    let options = self.cached_project_options.clone();
                    if let Some(opt) = options.get(idx) {
                        form.project_query = opt.search_text.clone();
                        form.selected_project_idx = Some(idx);
                    }
                }
                Task::none()
            }

            Message::FormHoursChanged(h) => {
                if let Some(form) = &mut self.entry_form {
                    form.hours_input = h;
                }
                Task::none()
            }

            Message::FormNotesChanged(n) => {
                if let Some(form) = &mut self.entry_form {
                    form.notes_input = n;
                }
                Task::none()
            }

            Message::FormFocusHours => {
                text_input::focus(text_input::Id::new("form_hours"))
            }

            Message::FormFocusNotes => {
                text_input::focus(text_input::Id::new("form_notes"))
            }

            Message::FormSubmit => {
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
                let opt = match form.selected_project_idx {
                    Some(idx) => options.get(idx).cloned(),
                    None => options
                        .iter()
                        .find(|o| {
                            o.search_text.to_lowercase()
                                == form.project_query.to_lowercase()
                        })
                        .cloned(),
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
                let _ = self.favorites.save(&self.settings.data_dir);
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
                        Message::EntryUpdated,
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
                        Message::EntryCreated,
                    )
                }
            }

            Message::EntryCreated(result) => {
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

            Message::EntryUpdated(result) => {
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

            Message::DeleteRequest(id) => {
                self.pending_delete = Some(id);
                Task::none()
            }

            Message::DeleteCancel => {
                self.pending_delete = None;
                Task::none()
            }

            Message::DeleteEntry(id) => {
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
                    Message::EntryDeleted,
                )
            }

            Message::EntryDeleted(result) => {
                match result {
                    Ok(id) => self.entries.retain(|e| e.id != id),
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            Message::TimerStart(id) => {
                let Some(client) = self.client.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move { client.restart_timer(id).await.map_err(|e| e.to_string()) },
                    Message::TimerStarted,
                )
            }

            Message::TimerStop(id) => {
                let Some(client) = self.client.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move { client.stop_timer(id).await.map_err(|e| e.to_string()) },
                    Message::TimerStopped,
                )
            }

            Message::TimerStarted(result) => {
                match result {
                    Ok(updated) => {
                        if let Some(e) = self.entries.iter_mut().find(|e| e.id == updated.id) {
                            *e = updated;
                        }
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            Message::TimerStopped(result) => {
                match result {
                    Ok(updated) => {
                        if let Some(e) = self.entries.iter_mut().find(|e| e.id == updated.id) {
                            *e = updated;
                        }
                    }
                    Err(e) => self.error_banner = Some(e),
                }
                Task::none()
            }

            Message::TemplateApply(idx) => {
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
                }
                form.hours_input = tpl.hours.clone();
                form.notes_input = tpl.notes.clone();
                Task::none()
            }

            _ => unreachable!(),
        }
    }
}
