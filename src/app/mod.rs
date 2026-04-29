use chrono::{Datelike, Local, NaiveDate};
use iced::font::Family;
use iced::widget::{button, container, row, text, Space};
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
use crate::state::overtime_adjustments::OvertimeAdjustmentStore;
#[cfg(not(target_os = "macos"))]
use crate::state::work_day::WorkPhase;
use crate::stats::{year_to_date_balance, HolidayStats, YearBalance};
use crate::ui::{billable_view, day_view, project_tracking_view, settings_view, stats_view, vacation_view};

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
mod project_tracking;

pub use billable::BillableMsg;
pub use billable::{BillablePageState, BillableSummary};
pub use stats::StatsMsg;
pub use stats::OvertimeAdjustmentForm;
pub use vacation::VacationMsg;
pub use vacation::{VacationForm, VacationPageState, VacationSummary};
pub use work_day::WorkDayMsg;
pub use work_day::WorkDayEditState;
pub use update::NavMsg;
pub use update::DatePickerState;
pub use entries::EntryMsg;
pub use entries::EntryForm;
pub use settings::SettingsMsg;
pub use settings::{SettingsFormState, ValidatedProfile, ValidatedCarryover, TemplateFormState};
pub use project_tracking::ProjectTrackingMsg;
pub use project_tracking::{ProjectTrackingPageState, BudgetSummary, BudgetForm};

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

/// Amber warning — used for "almost there" progress states.
pub const WARNING: Color = Color {
    r: 0.961,
    g: 0.620,
    b: 0.043,
    a: 1.0,
};

/// Root background — the darkest layer behind all content.
pub const BACKGROUND: Color = Color {
    r: 0.100,
    g: 0.110,
    b: 0.145,
    a: 1.0,
};

// ── Page ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Settings,
    Day,
    Stats,
    Vacation,
    Billable,
    ProjectTracking,
}

// ── Sub-state structs ────────────────────────────────────────────────────────
// Moved to their domain modules:
//   EntryForm          → entries.rs
//   VacationForm etc.  → vacation.rs
//   BillablePageState  → billable.rs
//   SettingsFormState   → settings.rs
//   WorkDayEditState   → work_day.rs
//   DatePickerState    → update.rs

// ── Message ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    // Font loading
    FontLoaded(Result<(), font::Error>),

    // Navigation / Date picker
    Nav(NavMsg),

    // Tray
    TrayToggle,
    TrayReady,
    TrayUnavailable,
    TrayMenuRefreshed,
    QuitApp,

    // Entries / Timer
    Entry(Box<EntryMsg>),

    // Work day
    WorkDay(WorkDayMsg),

    // Settings
    Settings(SettingsMsg),

    // Stats
    Stats(StatsMsg),

    // Vacation
    Vacation(VacationMsg),

    // Billable
    Billable(BillableMsg),

    // Project Tracking
    ProjectTracking(ProjectTrackingMsg),

    // Window lifecycle
    WindowIdReceived(Option<window::Id>),
    WindowCloseRequested(window::Id),

    // Current user — fetched on startup so we can filter time-entry requests
    CurrentUserLoaded(Result<i64, String>),

    // Focus
    TabPressed { shift: bool },
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct EasyHarvest {
    pub page: Page,
    pub settings: Settings,
    pub client: Option<HarvestClient>,
    /// Harvest user-id of the authenticated account.
    /// Used to filter `/time_entries` requests so that managers only see their
    /// own entries and not those of every member on their projects.
    pub harvest_user_id: Option<i64>,
    pub assignments: Vec<ProjectAssignment>,
    pub favorites: Favorites,
    pub current_date: NaiveDate,
    pub entries: Vec<TimeEntry>,
    pub entry_form: Option<EntryForm>,
    pub pending_delete: Option<i64>,
    pub work_day_store: WorkDayStore,
    pub year_balance: Option<YearBalance>,
    pub holiday_stats: Option<HolidayStats>,
    pub month_summaries: Option<Vec<crate::stats::MonthSummary>>,
    pub loading: bool,
    pub error_banner: Option<String>,

    // Generation counters — prevent stale async results from overwriting fresh data
    pub entries_gen: u64,
    pub vacation_gen: u64,
    pub billable_gen: u64,
    pub stats_gen: u64,
    pub project_tracking_gen: u64,

    // Sub-states
    pub settings_form: SettingsFormState,
    pub template_form: TemplateFormState,
    pub work_day_edit: WorkDayEditState,
    pub vacation: VacationPageState,
    pub billable: BillablePageState,
    pub project_tracking: ProjectTrackingPageState,
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
    pub overtime_adjustments: OvertimeAdjustmentStore,
    pub overtime_adj_form: Option<OvertimeAdjustmentForm>,

    // Cached computations
    pub cached_project_options: Vec<ProjectOption>,
    pub cached_expected_hours: f64,

    // Tray phase — shared with the tray thread so the context menu is phase-aware
    #[cfg(not(target_os = "macos"))]
    pub tray_phase: std::sync::Arc<std::sync::Mutex<WorkPhase>>,
    #[cfg(not(target_os = "macos"))]
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
            warning: ACCENT,
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
        // Intercept close on Linux/Windows so the tray can keep the process
        // (and tray icon) alive when the window is closed.
        exit_on_close_request: cfg!(target_os = "macos"),
        icon: window_icon(),
        ..Default::default()
    }
}

fn window_icon() -> Option<window::Icon> {
    // RGBA8 128×128 — crisp on 2× Retina; the OS downscales for standard DPI.
    const DATA: &[u8] = include_bytes!("../../assets/window_64.rgba8");
    window::icon::from_rgba(DATA.to_vec(), 128, 128).ok()
}

fn window_title(_state: &EasyHarvest, _window: window::Id) -> String {
    "Easy Harvest".to_string()
}

fn window_theme(_state: &EasyHarvest, _window: window::Id) -> Theme {
    app_theme()
}

pub fn run() -> iced::Result {
    // Use `daemon` instead of `application` so the process (and tray icon)
    // stays alive when the window is closed.  The initial window is opened
    // manually inside `EasyHarvest::new`.
    iced::daemon(EasyHarvest::new, EasyHarvest::update, EasyHarvest::view)
        .title(window_title)
        .theme(window_theme)
        .font(include_bytes!("../../assets/fonts/Inter-Regular.ttf").as_slice())
        .font(include_bytes!("../../assets/fonts/Inter-Medium.ttf").as_slice())
        .subscription(EasyHarvest::subscription)
        .run()
}

// ── Init ──────────────────────────────────────────────────────────────────────

impl EasyHarvest {
    pub(crate) fn new() -> (Self, Task<Message>) {
        #[cfg(debug_assertions)]
        let t0 = std::time::Instant::now();

        let settings = Settings::load(&BootstrapConfig::load().data_dir);
        #[cfg(debug_assertions)]
        eprintln!("[startup] Settings::load          {:?}", t0.elapsed());

        let today = Local::now().naive_local().date();
        let work_day_store =
            WorkDayStore::load(&settings.data_dir, today.year(), today.month());
        #[cfg(debug_assertions)]
        eprintln!("[startup] WorkDayStore::load       {:?}", t0.elapsed());

        let favorites = Favorites::load(&settings.data_dir);
        let templates = Templates::load(&settings.data_dir);
        let project_tracking = ProjectTrackingPageState::new(&settings.data_dir, today.year());
        let overtime_adjustments = OvertimeAdjustmentStore::load(&settings.data_dir);
        #[cfg(debug_assertions)]
        eprintln!("[startup] JSON loads done          {:?}", t0.elapsed());

        let token = Settings::load_token(&settings.data_dir);
        #[cfg(debug_assertions)]
        eprintln!("[startup] load_token (keyring)     {:?}", t0.elapsed());

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
        let init_pct = format!("{:.1}", settings.work_percentage * 100.0);
        let init_holidays = settings.total_holiday_days_per_year.to_string();
        let init_first_work_day = settings.first_work_day
            .map(|d| d.format("%d.%m.%Y").to_string())
            .unwrap_or_default();

        #[cfg(not(target_os = "macos"))]
        let initial_tray_phase = work_day_store.get_or_default(today).phase();

        let mut state = Self {
            page: initial_page.clone(),
            settings,
            client,
            harvest_user_id: None,
            assignments: Vec::new(),
            favorites,
            current_date: today,
            entries: Vec::new(),
            entry_form: None,
            pending_delete: None,
            work_day_store,
            year_balance: None,
            holiday_stats: None,
            month_summaries: None,
            loading: false,
            error_banner: None,
            entries_gen: 0,
            vacation_gen: 0,
            billable_gen: 0,
            stats_gen: 0,
            project_tracking_gen: 0,
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
            project_tracking,
            window_id: None,
            // Optimistically assume the tray works on Linux/Windows; set to false
            // only if the tray subscription reports a spawn failure.
            tray_available: cfg!(not(target_os = "macos")),
            window_visible: true,
            // Skip the data-folder step if the user has already configured it.
            wizard_step: if BootstrapConfig::config_path().exists() { 1 } else { 0 },
            overtime_year: today.year(),
            overtime_adjustments,
            overtime_adj_form: None,
            templates,
            cached_project_options: Vec::new(),
            cached_expected_hours: 0.0,
            #[cfg(not(target_os = "macos"))]
            tray_phase: std::sync::Arc::new(std::sync::Mutex::new(initial_tray_phase)),
            #[cfg(not(target_os = "macos"))]
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
                state.load_current_user_task(),
                open_task.map(|id| Message::WindowIdReceived(Some(id))),
            ])
        } else {
            open_task.map(|id| Message::WindowIdReceived(Some(id)))
        };

        #[cfg(debug_assertions)]
        eprintln!("[startup] EasyHarvest::new total   {:?}", t0.elapsed());

        (state, task)
    }
}
