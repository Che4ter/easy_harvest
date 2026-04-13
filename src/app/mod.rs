use chrono::{Datelike, Local, NaiveDate};
use iced::font::Family;
use iced::widget::{button, container, row, text, text_input, Space};
use iced::{
    self, font, keyboard, window, Color, Element, Font, Subscription, Task, Theme,
};

use crate::harvest::client::{HarvestClient, HarvestError};
use crate::harvest::models::{CreateTimeEntry, ProjectAssignment, TimeEntry, UpdateTimeEntry};
use crate::state::cache::ProjectCache;
use crate::state::favorites::{Favorites, ProjectOption};
use crate::state::persistence::WorkDayStore;
use crate::state::bootstrap::BootstrapConfig;
use crate::state::settings::{swiss_public_holidays, PublicHoliday, Settings};
use crate::state::templates::Templates;
#[cfg(target_os = "linux")]
use crate::state::work_day::WorkPhase;
use crate::stats::{year_to_date_balance, HolidayStats, YearBalance};
use crate::ui::{billable_view, day_view, settings_view, stats_view, vacation_view};

mod tasks;
mod update;
mod entries;
mod work_day;
mod settings;
mod vacation;
mod billable;
mod stats;
mod subscription;
mod view;
#[cfg(test)]
mod tests;

// ── Fonts ────────────────────────────────────────────────────────────────────

pub const FONT_REGULAR: Font = Font {
    family: Family::Name("Inter"),
    weight: iced::font::Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const FONT_MEDIUM: Font = Font {
    family: Family::Name("Inter"),
    weight: iced::font::Weight::Medium,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const FONT_SEMIBOLD: Font = Font {
    family: Family::Name("Inter"),
    weight: iced::font::Weight::Semibold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

// ── Palette ──────────────────────────────────────────────────────────────────

/// Harvest orange accent used throughout the UI.
pub const ACCENT: Color = Color {
    r: 1.0,
    g: 0.49,
    b: 0.0,
    a: 1.0,
};

pub const SURFACE: Color = Color {
    r: 0.122,
    g: 0.133,
    b: 0.173,
    a: 1.0,
};

pub const SURFACE_RAISED: Color = Color {
    r: 0.165,
    g: 0.180,
    b: 0.231,
    a: 1.0,
};

pub const SURFACE_HOVER: Color = Color {
    r: 0.200,
    g: 0.215,
    b: 0.270,
    a: 1.0,
};

pub const TEXT_PRIMARY: Color = Color {
    r: 0.886,
    g: 0.898,
    b: 0.933,
    a: 1.0,
};

pub const TEXT_MUTED: Color = Color {
    r: 0.529,
    g: 0.557,
    b: 0.647,
    a: 1.0,
};

pub const SUCCESS: Color = Color {
    r: 0.369,
    g: 0.820,
    b: 0.545,
    a: 1.0,
};

pub const DANGER: Color = Color {
    r: 0.957,
    g: 0.357,
    b: 0.412,
    a: 1.0,
};

/// Root background — the darkest layer behind all content.
pub const BACKGROUND: Color = Color {
    r: 0.100,
    g: 0.110,
    b: 0.145,
    a: 1.0,
};

// ── Vacation form ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VacationForm {
    pub from_input: String,
    pub to_input: String,
    /// true = full day, false = half day
    pub full_day: bool,
    pub error: Option<String>,
    pub submitting: bool,
}

impl VacationForm {
    pub fn new() -> Self {
        Self {
            from_input: String::new(),
            to_input: String::new(),
            full_day: true,
            error: None,
            submitting: false,
        }
    }
}

impl Default for VacationForm {
    fn default() -> Self {
        Self::new()
    }
}

// ── Page ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Settings,
    Day,
    Stats,
    Vacation,
    Billable,
}

// ── Entry form ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EntryForm {
    /// None = create new; Some(id) = editing existing
    pub editing_id: Option<i64>,
    pub project_query: String,
    pub selected_project_idx: Option<usize>,
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
            hours_input: String::new(),
            notes_input: String::new(),
            error: None,
        }
    }

    pub fn for_entry(entry: &TimeEntry) -> Self {
        Self {
            editing_id: Some(entry.id),
            project_query: format!(
                "{} — {}",
                entry.project.name, entry.task.name
            ),
            selected_project_idx: None,
            hours_input: format!("{:.2}", entry.hours),
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

// ── Sub-state structs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SettingsFormState {
    pub token_input: String,
    pub account_input: String,
    pub connecting: bool,
    pub error: Option<String>,
    pub weekly_hours_input: String,
    pub percentage_input: String,
    pub holidays_input: String,
    pub first_work_day_input: String,
    pub profile_saved: bool,
    pub profile_error: Option<String>,
    pub carryover_year_input: String,
    pub carryover_holiday_input: String,
    pub carryover_overtime_input: String,
    pub carryover_error: Option<String>,
    pub holiday_view_year: i32,
    pub cached_holidays: Vec<PublicHoliday>,
    pub holiday_task_query: String,
    pub data_dir_input: String,
    pub data_dir_saved: bool,
    /// Cached deduped task list for holiday_tasks_section: (task_id, task_name, context).
    pub cached_task_list: Vec<(i64, String, String)>,
}

impl SettingsFormState {
    pub fn new(year: i32) -> Self {
        Self {
            holiday_view_year: year,
            cached_holidays: swiss_public_holidays(year),
            ..Default::default()
        }
    }

    pub fn validate_profile(&self) -> Result<ValidatedProfile, String> {
        let weekly_hours: f64 = self.weekly_hours_input.replace(',', ".").parse()
            .ok()
            .filter(|&v: &f64| v > 0.0 && v <= 168.0)
            .ok_or_else(|| "Invalid weekly hours (must be 1\u{2013}168)".to_string())?;
        let percentage: f64 = self.percentage_input.replace(',', ".").parse::<f64>()
            .ok()
            .filter(|&v| v > 0.0 && v <= 100.0)
            .map(|v| v / 100.0)
            .ok_or_else(|| "Invalid percentage (1\u{2013}100)".to_string())?;
        let holidays: u32 = self.holidays_input.parse::<u32>()
            .ok()
            .filter(|&v| v <= 365)
            .ok_or_else(|| "Invalid holiday days".to_string())?;
        let raw = self.first_work_day_input.trim();
        let first_work_day = if raw.is_empty() {
            None
        } else {
            Some(NaiveDate::parse_from_str(raw, "%d.%m.%Y")
                .map_err(|_| "Invalid first work day \u{2014} use DD.MM.YYYY".to_string())?)
        };
        Ok(ValidatedProfile { weekly_hours, percentage, holidays, first_work_day })
    }

    pub fn validate_carryover(&self) -> Result<ValidatedCarryover, String> {
        let year: i32 = self.carryover_year_input.trim().parse()
            .ok()
            .filter(|y: &i32| (2000..=2100).contains(y))
            .ok_or_else(|| "Invalid year (2000\u{2013}2100)".to_string())?;
        let holiday_days: f64 = self.carryover_holiday_input.replace(',', ".").parse()
            .map_err(|_| "Invalid holiday days".to_string())?;
        let overtime_hours: f64 = self.carryover_overtime_input.replace(',', ".").parse()
            .map_err(|_| "Invalid overtime hours".to_string())?;
        Ok(ValidatedCarryover { year, holiday_days, overtime_hours })
    }
}

pub struct ValidatedProfile {
    pub weekly_hours: f64,
    pub percentage: f64,
    pub holidays: u32,
    pub first_work_day: Option<NaiveDate>,
}

pub struct ValidatedCarryover {
    pub year: i32,
    pub holiday_days: f64,
    pub overtime_hours: f64,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateFormState {
    pub open: bool,
    pub label: String,
    pub project_query: String,
    pub project_idx: Option<usize>,
    pub hours: String,
    pub notes: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkDayEditState {
    pub edit_mode: bool,
    pub start_input: String,
    pub end_input: String,
    /// (break_start_str, break_end_str) per break in edit mode
    pub break_inputs: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct VacationSummary {
    pub used_days: f64,
    pub booked_days: f64,
    pub days_remaining: f64,
    pub total_days: f64,
    pub carryover_days: f64,
}

#[derive(Debug, Clone)]
pub struct VacationPageState {
    pub entries: Vec<TimeEntry>,
    pub year: i32,
    pub form: Option<VacationForm>,
    pub summary: Option<VacationSummary>,
}

impl VacationPageState {
    pub fn new(year: i32) -> Self {
        Self {
            entries: Vec::new(),
            year,
            form: None,
            summary: None,
        }
    }
}

impl Default for VacationPageState {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone)]
pub struct BillableSummary {
    pub total_hours: f64,
    pub billable_hours: f64,
    pub non_billable_hours: f64,
    pub billable_pct: f64,
    pub projects: Vec<(String, String, f64, f64)>, // (project_name, client_name, billable_h, total_h) sorted by billable desc
}

#[derive(Debug, Clone)]
pub struct BillablePageState {
    pub entries: Vec<TimeEntry>,
    pub year: i32,
    /// None = full year view; Some(m) = single month 1–12
    pub month: Option<u32>,
    pub summary: Option<BillableSummary>,
}

impl BillablePageState {
    pub fn new(year: i32) -> Self {
        Self {
            entries: Vec::new(),
            year,
            month: None,
            summary: None,
        }
    }
}

impl Default for BillablePageState {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DatePickerState {
    pub open: bool,
    pub month: NaiveDate,
}

impl DatePickerState {
    pub fn new(month: NaiveDate) -> Self {
        Self { open: false, month }
    }
}

// ── Message ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    // Font loading
    FontLoaded(Result<(), font::Error>),

    // Date picker
    DatePickerToggle,
    DatePickerMonthPrev,
    DatePickerMonthNext,
    DatePickerSelect(NaiveDate),

    // Navigation
    PageChanged(Page),
    DatePrev,
    DateNext,
    DateToday,

    // Tray
    TrayToggle,
    TrayReady,
    TrayUnavailable,
    TrayMenuRefreshed,
    QuitApp,

    // Data loading
    EntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    AssignmentsLoaded(Result<Vec<ProjectAssignment>, String>),
    SyncAssignments,
    StatsLoaded(u64, Result<(YearBalance, HolidayStats), String>),

    // Entry CRUD
    ShowAddForm,
    EditEntry(i64),
    CancelForm,
    FormProjectQueryChanged(String),
    FormProjectSelected(usize),
    FormHoursChanged(String),
    FormNotesChanged(String),
    FormFocusHours,
    FormFocusNotes,
    FormSubmit,
    EntryCreated(Result<TimeEntry, String>),
    EntryUpdated(Result<TimeEntry, String>),
    DeleteRequest(i64),
    DeleteCancel,
    DeleteEntry(i64),
    EntryDeleted(Result<i64, String>),

    // Entry timer
    TimerStart(i64),
    TimerStop(i64),
    TimerStarted(Result<TimeEntry, String>),
    TimerStopped(Result<TimeEntry, String>),

    // Work day
    StartDay,
    StartBreak,
    EndBreak,
    EndDay,
    ResumeDay,
    WorkDayTick,
    WorkDayEditStart,
    WorkDayEditCancel,
    WorkDayStartInputChanged(String),
    WorkDayEndInputChanged(String),
    WorkDayBreakStartChanged(usize, String),
    WorkDayBreakEndChanged(usize, String),
    WorkDayBreakDelete(usize),
    WorkDayBreakAdd,
    WorkDayEditSave,

    // Settings — connection
    SettingsTokenChanged(String),
    SettingsAccountIdChanged(String),
    SettingsSave,
    SettingsConnected(Result<String, String>),

    // Settings — work profile
    SettingsWeeklyHoursChanged(String),
    SettingsPercentageChanged(String),
    SettingsHolidaysChanged(String),
    SettingsFirstWorkDayChanged(String),
    SettingsSaveProfile,

    // Settings — carryover (year-indexed)
    SettingsCarryoverYearChanged(String),
    SettingsCarryoverHolidayChanged(String),
    SettingsCarryoverOvertimeChanged(String),
    SettingsCarryoverSave,
    SettingsCarryoverDelete(i32),

    // Settings — holiday tasks
    HolidayTaskToggle(i64),
    HolidayTaskQueryChanged(String),

    // Settings — data directory
    SettingsDataDirChanged(String),
    SettingsPickDataDir,
    SettingsDataDirPicked(Option<std::path::PathBuf>),
    SettingsSaveDataDir,

    // Settings — holiday list year navigation
    HolidayViewYearPrev,
    HolidayViewYearNext,

    // Stats
    StatsRefresh,
    OvertimeYearPrev,
    OvertimeYearNext,

    // Vacation
    VacationRefresh,
    VacationYearPrev,
    VacationYearNext,
    VacationEntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    VacationShowForm,
    VacationHideForm,
    VacationFromChanged(String),
    VacationToChanged(String),
    VacationDayTypeFull,
    VacationDayTypeHalf,
    VacationFormSubmit,
    VacationEntriesCreated(Result<Vec<TimeEntry>, String>),
    VacationDeleteEntry(i64),
    VacationEntryDeleted(Result<i64, String>),

    // Billable
    BillableRefresh,
    BillableYearPrev,
    BillableYearNext,
    BillableEntriesLoaded(u64, Result<Vec<TimeEntry>, String>),
    BillableMonthSelected(u32),
    BillableMonthClear,

    // Templates (entry quick-fill)
    TemplateApply(usize),
    SettingsTemplateAddOpen,
    SettingsTemplateAddCancel,
    SettingsTemplateAddLabelChanged(String),
    SettingsTemplateAddProjectQueryChanged(String),
    SettingsTemplateAddProjectSelected(usize),
    SettingsTemplateAddHoursChanged(String),
    SettingsTemplateAddNotesChanged(String),
    SettingsTemplateAddSave,
    SettingsTemplateDelete(usize),

    // Window lifecycle
    WindowIdReceived(Option<window::Id>),
    WindowCloseRequested(window::Id),

    // Wizard
    WizardNext,
    WizardBack,

    // Auth
    Disconnect,

    // Focus
    TabPressed { shift: bool },
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct EasyHarvest {
    pub page: Page,
    pub settings: Settings,
    pub client: Option<HarvestClient>,
    pub assignments: Vec<ProjectAssignment>,
    pub favorites: Favorites,
    pub current_date: NaiveDate,
    pub entries: Vec<TimeEntry>,
    pub entry_form: Option<EntryForm>,
    pub pending_delete: Option<i64>,
    pub work_day_store: WorkDayStore,
    pub year_balance: Option<YearBalance>,
    pub holiday_stats: Option<HolidayStats>,
    pub loading: bool,
    pub error_banner: Option<String>,

    // Generation counters — prevent stale async results from overwriting fresh data
    pub entries_gen: u64,
    pub vacation_gen: u64,
    pub billable_gen: u64,
    pub stats_gen: u64,

    // Sub-states
    pub settings_form: SettingsFormState,
    pub template_form: TemplateFormState,
    pub work_day_edit: WorkDayEditState,
    pub vacation: VacationPageState,
    pub billable: BillablePageState,
    pub date_picker: DatePickerState,

    pub window_id: Option<window::Id>,
    pub tray_available: bool,
    pub window_visible: bool,

    /// 0 = data-folder step, 1 = credentials step (first-run wizard only).
    pub wizard_step: u8,

    // Templates
    pub templates: Templates,

    // Overtime tab
    pub overtime_year: i32,

    // Cached computations
    pub cached_project_options: Vec<ProjectOption>,
    pub cached_expected_hours: f64,

    // Tray phase (Linux only — shared with the ksni tray so menu() is phase-aware)
    #[cfg(target_os = "linux")]
    pub tray_phase: std::sync::Arc<std::sync::Mutex<WorkPhase>>,
    #[cfg(target_os = "linux")]
    pub tray_update_notify: std::sync::Arc<tokio::sync::Notify>,
}

// ── Theme ─────────────────────────────────────────────────────────────────────

fn app_theme() -> Theme {
    Theme::custom(
        "EasyHarvest".to_string(),
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT_PRIMARY,
            primary: ACCENT,
            success: SUCCESS,
            danger: DANGER,
        },
    )
}

// ── Run ───────────────────────────────────────────────────────────────────────

fn window_settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(520.0, 720.0),
        resizable: true,
        decorations: true,
        // Intercept close on Linux so the tray can close the window while
        // keeping the process (and tray icon) alive.
        exit_on_close_request: !cfg!(target_os = "linux"),
        icon: window_icon(),
        ..Default::default()
    }
}

fn window_icon() -> Option<window::Icon> {
    // RGBA8 64×64 data pre-computed at build time by build.rs.
    const DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/window_64.rgba8"));
    window::icon::from_rgba(DATA.to_vec(), 64, 64).ok()
}

pub fn run() -> iced::Result {
    // Use `daemon` instead of `application` so the process (and tray icon)
    // stays alive when the window is closed.  The initial window is opened
    // manually inside `EasyHarvest::new`.
    iced::daemon("Easy Harvest", EasyHarvest::update, EasyHarvest::view)
        .theme(|_state, _window| app_theme())
        .font(include_bytes!("../../assets/fonts/Inter-Regular.ttf").as_slice())
        .font(include_bytes!("../../assets/fonts/Inter-Medium.ttf").as_slice())
        .subscription(EasyHarvest::subscription)
        .run_with(EasyHarvest::new)
}

// ── Init ──────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn new() -> (Self, Task<Message>) {
        let settings = Settings::load(&BootstrapConfig::load().data_dir);
        let today = Local::now().naive_local().date();
        let work_day_store =
            WorkDayStore::load(&settings.data_dir, today.year(), today.month());
        let favorites = Favorites::load(&settings.data_dir);
        let templates = Templates::load(&settings.data_dir);

        let token = Settings::load_token(&settings.data_dir);
        let client = token.as_ref().and_then(|t| {
            HarvestClient::new(t.clone(), settings.account_id.clone()).ok()
        });

        let initial_page = if client.is_some() && !settings.account_id.is_empty() {
            Page::Day
        } else {
            Page::Settings
        };

        let init_data_dir = settings.data_dir.display().to_string();
        let init_weekly = settings.total_weekly_hours.to_string();
        let init_pct = format!("{}", (settings.work_percentage * 100.0).round() as u32);
        let init_holidays = settings.total_holiday_days_per_year.to_string();
        let init_first_work_day = settings.first_work_day
            .map(|d| d.format("%d.%m.%Y").to_string())
            .unwrap_or_default();

        #[cfg(target_os = "linux")]
        let initial_tray_phase = work_day_store.get_or_default(today).phase();

        let mut state = Self {
            page: initial_page.clone(),
            settings,
            client,
            assignments: Vec::new(),
            favorites,
            current_date: today,
            entries: Vec::new(),
            entry_form: None,
            pending_delete: None,
            work_day_store,
            year_balance: None,
            holiday_stats: None,
            loading: false,
            error_banner: None,
            entries_gen: 0,
            vacation_gen: 0,
            billable_gen: 0,
            stats_gen: 0,
            settings_form: SettingsFormState {
                weekly_hours_input: init_weekly,
                percentage_input: init_pct,
                holidays_input: init_holidays,
                first_work_day_input: init_first_work_day,
                holiday_view_year: today.year(),
                data_dir_input: init_data_dir,
                ..SettingsFormState::new(today.year())
            },
            template_form: TemplateFormState::default(),
            work_day_edit: WorkDayEditState::default(),
            vacation: VacationPageState::new(today.year()),
            date_picker: DatePickerState::new(today),
            billable: BillablePageState::new(today.year()),
            window_id: None,
            // Optimistically assume the tray works on Linux; set to false
            // only if the tray subscription reports a spawn failure.
            tray_available: cfg!(target_os = "linux"),
            window_visible: true,
            // Skip the data-folder step if the user has already configured it.
            wizard_step: if BootstrapConfig::config_path().exists() { 1 } else { 0 },
            overtime_year: today.year(),
            templates,
            cached_project_options: Vec::new(),
            cached_expected_hours: 0.0,
            #[cfg(target_os = "linux")]
            tray_phase: std::sync::Arc::new(std::sync::Mutex::new(initial_tray_phase)),
            #[cfg(target_os = "linux")]
            tray_update_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
        };

        // Open the initial window.  With `iced::daemon` windows must be opened
        // manually; the daemon does not create one automatically.
        state.recompute_expected_hours();
        let (win_id, open_task) = window::open(window_settings());
        state.window_id = Some(win_id);

        let task = if initial_page == Page::Day {
            state.loading = true;
            Task::batch([
                state.load_entries_task(),
                state.load_assignments_task(),
                open_task.map(|id| Message::WindowIdReceived(Some(id))),
            ])
        } else {
            open_task.map(|id| Message::WindowIdReceived(Some(id)))
        };

        (state, task)
    }
}
