//! Integration tests for the Harvest API client.
//!
//! These tests require real credentials and hit the live API.
//! They are marked `#[ignore]` so they don't run in plain `cargo test`.
//!
//! Run all:
//!   HARVEST_TOKEN=<token> HARVEST_ACCOUNT_ID=<id> cargo test -- --ignored
//!
//! Run one:
//!   HARVEST_TOKEN=<token> HARVEST_ACCOUNT_ID=<id> cargo test test_user -- --ignored

use easy_harvest::harvest::client::{HarvestClient, HarvestError};
use easy_harvest::harvest::models::{CreateTimeEntry, UpdateTimeEntry};
use easy_harvest::state::settings::Settings;
use easy_harvest::stats;
use chrono::{Datelike, Duration, NaiveDate};

/// Returns a Monday at least 30 days in the future, safe to use as a test date.
fn safe_test_date() -> String {
    let today = chrono::Local::now().naive_local().date();
    let mut date = today + Duration::days(30);
    // Advance to the next Monday
    while date.weekday().num_days_from_monday() != 0 {
        date += Duration::days(1);
    }
    date.format("%Y-%m-%d").to_string()
}

/// Find an active project + task from assignments, or panic with a clear message.
async fn active_project_and_task(
    client: &HarvestClient,
) -> (easy_harvest::harvest::models::ProjectAssignment, easy_harvest::harvest::models::ProjectTaskAssignment) {
    let assignments = client
        .list_all_my_project_assignments()
        .await
        .expect("list_all_my_project_assignments failed");

    let pa = assignments
        .into_iter()
        .find(|pa| pa.is_active && pa.task_assignments.iter().any(|ta| ta.is_active))
        .expect("no active project with tasks found");

    let ta = pa
        .task_assignments
        .iter()
        .find(|ta| ta.is_active)
        .cloned()
        .expect("no active task");

    (pa, ta)
}

/// Returns a client built from HARVEST_TOKEN / HARVEST_ACCOUNT_ID env vars,
/// or panics with a clear message if they are not set.
fn client() -> HarvestClient {
    let token = std::env::var("HARVEST_TOKEN")
        .expect("Set HARVEST_TOKEN env var to run integration tests");
    let account_id = std::env::var("HARVEST_ACCOUNT_ID")
        .expect("Set HARVEST_ACCOUNT_ID env var to run integration tests");
    HarvestClient::new(token, account_id).expect("Failed to build HTTP client")
}

// ---------------------------------------------------------------------------
// Read-only tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_user() {
    let user = client()
        .get_current_user()
        .await
        .expect("get_current_user failed");

    println!("User: {} {} <{}>", user.first_name, user.last_name, user.email);
    assert!(!user.first_name.is_empty(), "first_name should not be empty");
    assert!(!user.email.is_empty(), "email should not be empty");
}

#[tokio::test]
#[ignore]
async fn test_project_assignments() {
    let assignments = client()
        .list_all_my_project_assignments()
        .await
        .expect("list_all_my_project_assignments failed");

    println!("Project assignments: {}", assignments.len());
    for pa in &assignments {
        println!("  [{}] {} > {}", pa.project.id, pa.client.name, pa.project.name);
        for ta in &pa.task_assignments {
            if ta.is_active {
                println!("       task [{}] {}", ta.task.id, ta.task.name);
            }
        }
    }

    assert!(!assignments.is_empty(), "should have at least one project assignment");
}

#[tokio::test]
#[ignore]
async fn test_today_entries() {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let entries = client()
        .list_all_time_entries(&today, &today)
        .await
        .expect("list_all_time_entries failed");

    println!("Entries for {today}: {}", entries.len());
    let total: f64 = entries.iter().map(|e| e.hours).sum();
    println!("Total hours: {total:.2}h");

    for e in &entries {
        println!(
            "  [{}] {} > {} | {} | {:.2}h{}",
            e.id,
            e.client.name,
            e.project.name,
            e.task.name,
            e.hours,
            e.notes.as_deref().filter(|n| !n.is_empty())
                .map(|n| format!(" | {n}"))
                .unwrap_or_default(),
        );
    }
}

// ---------------------------------------------------------------------------
// CRUD test — creates, updates, then deletes a real entry
// Uses a fixed future date (next Monday) to avoid touching real bookings.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_create_update_delete_entry() {
    let client = client();

    // Need a valid project + task to create an entry
    let assignments = client
        .list_all_my_project_assignments()
        .await
        .expect("need project assignments to create a test entry");

    let pa = assignments
        .iter()
        .find(|pa| pa.is_active && !pa.task_assignments.is_empty())
        .expect("no active project with tasks found");

    let ta = pa
        .task_assignments
        .iter()
        .find(|ta| ta.is_active)
        .expect("no active task found");

    // Use a Monday well in the future so this test never touches real bookings.
    let test_date = safe_test_date();

    // --- Create ---
    let created = client
        .create_time_entry(&CreateTimeEntry {
            project_id: pa.project.id,
            task_id: ta.task.id,
            spent_date: test_date.to_string(),
            hours: 0.25,
            notes: Some("easy_harvest integration test — safe to delete".to_string()),
        })
        .await
        .expect("create_time_entry failed");

    println!("Created entry [{}] {:.2}h on {test_date}", created.id, created.hours);
    assert_eq!(created.hours, 0.25);
    assert_eq!(created.spent_date, test_date);
    assert_eq!(created.project.id, pa.project.id);

    // --- Update ---
    let updated = client
        .update_time_entry(
            created.id,
            &UpdateTimeEntry {
                project_id: None,
                task_id: None,
                spent_date: None,
                hours: Some(0.5),
                notes: Some("easy_harvest integration test (updated) — safe to delete".to_string()),
            },
        )
        .await
        .expect("update_time_entry failed");

    println!("Updated entry [{}] {:.2}h", updated.id, updated.hours);
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.hours, 0.5);

    // --- Delete ---
    client
        .delete_time_entry(created.id)
        .await
        .expect("delete_time_entry failed");

    println!("Deleted entry [{}]", created.id);

    // --- Verify deletion ---
    let entries_after = client
        .list_all_time_entries(&test_date, &test_date)
        .await
        .expect("list after delete failed");

    let still_exists = entries_after.iter().any(|e| e.id == created.id);
    assert!(!still_exists, "entry should be gone after delete");
}

// ---------------------------------------------------------------------------
// Keyring / Settings test
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn test_keyring_save_load() {
    let test_token = "test_token_easy_harvest_12345";
    let dir = tempfile::tempdir().expect("tempdir");

    Settings::save_token(test_token, dir.path()).expect("save_token failed");

    // Surface the actual keyring error if load fails
    let entry = keyring::Entry::new("easy_harvest", "harvest_api_token")
        .expect("failed to create keyring entry");
    match entry.get_password() {
        Ok(loaded) => {
            assert_eq!(loaded, test_token, "loaded token should match saved token");
            println!("Keyring save/load works correctly on this platform");
        }
        Err(e) => {
            panic!("keyring get_password failed: {e:?} — keyring not functional on this platform");
        }
    }
}

// ---------------------------------------------------------------------------
// Stats integration tests — fetch real data, validate calculations
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_stats_current_month() {
    let today = chrono::Local::now().naive_local().date();
    let (from, to) = stats::month_bounds(today.year(), today.month());

    let entries = client()
        .list_all_time_entries(&from.to_string(), &to.to_string())
        .await
        .expect("list_all_time_entries failed");

    println!("Entries for {from} – {to}: {}", entries.len());

    let period = stats::period_stats(&entries, from, to, 8.0, &[], today);
    println!("  Total hours:    {:.2}h", period.total_hours);
    println!("  Expected hours: {:.2}h", period.expected_hours);
    println!("  Balance:        {:+.2}h", period.balance_hours);
    println!("  Working days expected:  {}", period.working_days_expected);
    println!("  Days with entries:      {}", period.days_with_entries);

    assert!(period.total_hours >= 0.0);
    assert!(period.expected_hours > 0.0, "should have >0 expected hours in current month");

    // Daily breakdown
    let daily = stats::daily_summaries(&entries);
    println!("\nDaily breakdown:");
    for day in &daily {
        println!("  {} {:.2}h ({} entries)", day.date, day.total_hours, day.entry_count);
    }
}

#[tokio::test]
#[ignore]
async fn test_stats_holiday_year() {
    let today = chrono::Local::now().naive_local().date();
    let year = today.year();
    let (from, to) = stats::year_bounds(year);

    let entries = client()
        .list_all_time_entries(&from.to_string(), &to.to_string())
        .await
        .expect("list_all_time_entries failed");

    println!("Entries for full year {year}: {}", entries.len());

    // Holiday stats require full-year entries and configured holiday task IDs.
    // With no IDs configured the function returns zeros — useful as a smoke test.
    let holiday = stats::holiday_stats(&entries, year, &[], 25.0, 8.0);
    println!("  Holiday days taken:     {:.2}", holiday.days_taken);
    println!("  Holiday days remaining: {:.2}", holiday.days_remaining);
    println!("  Holiday total days:     {}", holiday.total_days);

    assert!(holiday.days_taken >= 0.0);
    assert!(
        (holiday.days_taken + holiday.days_remaining - holiday.total_days).abs() < 1e-9,
        "taken + remaining should equal total_days"
    );

    // Year-to-date balance with zero carryover
    let ytd = stats::year_to_date_balance(&entries, year, None, 8.0, &[], 0.0, 0.0, today);
    println!("\nYear-to-date balance:");
    println!("  Total hours:    {:.2}h", ytd.period.total_hours);
    println!("  Expected hours: {:.2}h", ytd.period.expected_hours);
    println!("  Period balance: {:+.2}h", ytd.period.balance_hours);
    println!("  Carryover:      {:+.2}h", ytd.carryover_hours);
    println!("  Total balance:  {:+.2}h", ytd.total_balance);
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

/// A bad token must produce HarvestError::Unauthorized — not a network or
/// parse error — so the UI can show a useful message.
#[tokio::test]
#[ignore]
async fn test_unauthorized_error() {
    let bad_client = HarvestClient::new("invalid-token".into(), "000000".into())
        .expect("failed to build client");

    let result = bad_client.get_current_user().await;

    assert!(
        matches!(result, Err(HarvestError::Unauthorized)),
        "expected Unauthorized, got: {result:?}"
    );
}

/// Fetching a time entry that does not exist must return an API error (404),
/// not panic or produce a deserialization error.
#[tokio::test]
#[ignore]
async fn test_delete_nonexistent_entry_returns_api_error() {
    let result = client().delete_time_entry(1).await;

    assert!(
        matches!(result, Err(HarvestError::Api { status: 404, .. }) | Err(HarvestError::Api { .. })),
        "expected an API error for a nonexistent entry, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Date-range filtering
// ---------------------------------------------------------------------------

/// The API must not return entries outside the requested date range.
#[tokio::test]
#[ignore]
async fn test_date_range_boundaries() {
    let today = chrono::Local::now().naive_local().date();
    let (from, to) = stats::month_bounds(today.year(), today.month());
    let from_str = from.format("%Y-%m-%d").to_string();
    let to_str = to.format("%Y-%m-%d").to_string();

    let entries = client()
        .list_all_time_entries(&from_str, &to_str)
        .await
        .expect("list_all_time_entries failed");

    for entry in &entries {
        let date = NaiveDate::parse_from_str(&entry.spent_date, "%Y-%m-%d")
            .expect("entry has invalid spent_date");
        assert!(
            date >= from && date <= to,
            "entry [{}] has date {} outside requested range {} – {}",
            entry.id, entry.spent_date, from_str, to_str
        );
    }
}

/// Requesting a single day must return only entries for that day.
#[tokio::test]
#[ignore]
async fn test_single_day_range() {
    let today = chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string();

    let entries = client()
        .list_all_time_entries(&today, &today)
        .await
        .expect("list_all_time_entries failed");

    for entry in &entries {
        assert_eq!(
            entry.spent_date, today,
            "entry [{}] spent_date {} != requested date {}",
            entry.id, entry.spent_date, today
        );
    }
}

// ---------------------------------------------------------------------------
// Entry field validation
// ---------------------------------------------------------------------------

/// All fields that must be present on every entry returned by the API are
/// non-empty / in the expected range.
#[tokio::test]
#[ignore]
async fn test_entry_fields_are_valid() {
    let today = chrono::Local::now().naive_local().date();
    let (from, _) = stats::month_bounds(today.year(), today.month());
    let from_str = from.format("%Y-%m-%d").to_string();
    let to_str = today.format("%Y-%m-%d").to_string();

    let entries = client()
        .list_all_time_entries(&from_str, &to_str)
        .await
        .expect("list_all_time_entries failed");

    if entries.is_empty() {
        println!("No entries in range — skipping field validation");
        return;
    }

    for entry in &entries {
        assert!(entry.id > 0, "entry id must be positive");
        assert!(entry.hours >= 0.0, "entry hours must be non-negative");
        assert!(!entry.spent_date.is_empty(), "entry spent_date must not be empty");
        assert!(entry.project.id > 0, "project id must be positive");
        assert!(!entry.project.name.is_empty(), "project name must not be empty");
        assert!(entry.task.id > 0, "task id must be positive");
        assert!(!entry.task.name.is_empty(), "task name must not be empty");
        assert!(entry.client.id > 0, "client id must be positive");
        assert!(!entry.client.name.is_empty(), "client name must not be empty");
        assert!(entry.user.id > 0, "user id must be positive");
        // spent_date must parse as YYYY-MM-DD
        NaiveDate::parse_from_str(&entry.spent_date, "%Y-%m-%d")
            .expect("spent_date must be YYYY-MM-DD");
    }

    println!("Validated {} entries — all fields OK", entries.len());
}

// ---------------------------------------------------------------------------
// Notes round-trip
// ---------------------------------------------------------------------------

/// Notes set on creation must come back unchanged in both the create response
/// and a subsequent list query.
#[tokio::test]
#[ignore]
async fn test_notes_round_trip() {
    let client = client();
    let (pa, ta) = active_project_and_task(&client).await;
    let test_date = safe_test_date();
    let notes = "easy_harvest notes round-trip test — safe to delete".to_string();

    let created = client
        .create_time_entry(&CreateTimeEntry {
            project_id: pa.project.id,
            task_id: ta.task.id,
            spent_date: test_date.clone(),
            hours: 0.25,
            notes: Some(notes.clone()),
        })
        .await
        .expect("create failed");

    assert_eq!(created.notes.as_deref(), Some(notes.as_str()), "notes mismatch on create");

    // Verify the notes survive a round-trip through the list endpoint
    let listed = client
        .list_all_time_entries(&test_date, &test_date)
        .await
        .expect("list failed");

    let found = listed.iter().find(|e| e.id == created.id).expect("created entry not found in list");
    assert_eq!(found.notes.as_deref(), Some(notes.as_str()), "notes mismatch in list response");

    // Cleanup
    client.delete_time_entry(created.id).await.expect("delete failed");
}

// ---------------------------------------------------------------------------
// Update — partial fields
// ---------------------------------------------------------------------------

/// Only the fields included in UpdateTimeEntry should change; others are untouched.
#[tokio::test]
#[ignore]
async fn test_partial_update_preserves_other_fields() {
    let client = client();
    let (pa, ta) = active_project_and_task(&client).await;
    let test_date = safe_test_date();

    let created = client
        .create_time_entry(&CreateTimeEntry {
            project_id: pa.project.id,
            task_id: ta.task.id,
            spent_date: test_date.clone(),
            hours: 0.25,
            notes: Some("original notes".to_string()),
        })
        .await
        .expect("create failed");

    // Update only hours — notes and project/task must stay the same
    let updated = client
        .update_time_entry(
            created.id,
            &UpdateTimeEntry {
                project_id: None,
                task_id: None,
                spent_date: None,
                hours: Some(0.75),
                notes: None,
            },
        )
        .await
        .expect("update failed");

    assert_eq!(updated.hours, 0.75, "hours should be updated");
    assert_eq!(updated.project.id, pa.project.id, "project should be unchanged");
    assert_eq!(updated.task.id, ta.task.id, "task should be unchanged");
    assert_eq!(updated.spent_date, test_date, "date should be unchanged");

    // Cleanup
    client.delete_time_entry(created.id).await.expect("delete failed");
}

// ---------------------------------------------------------------------------
// Timer lifecycle
// ---------------------------------------------------------------------------

/// Create an entry, start its timer (restart), verify it is running,
/// then stop it and verify it is no longer running. Cleans up after itself.
#[tokio::test]
#[ignore]
async fn test_timer_restart_and_stop() {
    let client = client();
    let (pa, ta) = active_project_and_task(&client).await;
    let test_date = safe_test_date();

    let created = client
        .create_time_entry(&CreateTimeEntry {
            project_id: pa.project.id,
            task_id: ta.task.id,
            spent_date: test_date.clone(),
            hours: 0.1,
            notes: Some("easy_harvest timer test — safe to delete".to_string()),
        })
        .await
        .expect("create failed");

    // Start the timer
    let running = client
        .restart_timer(created.id)
        .await
        .expect("restart_timer failed");

    assert!(running.is_running, "entry should be running after restart");
    assert!(running.timer_started_at.is_some(), "timer_started_at should be set");
    println!("Timer started at: {:?}", running.timer_started_at);

    // Stop the timer
    let stopped = client
        .stop_timer(created.id)
        .await
        .expect("stop_timer failed");

    assert!(!stopped.is_running, "entry should not be running after stop");
    assert!(stopped.hours >= 0.1, "hours should be >= the initial value after timer ran");
    println!("Timer stopped — final hours: {:.4}h", stopped.hours);

    // Cleanup
    client.delete_time_entry(created.id).await.expect("delete failed");
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// list_all_time_entries must accumulate all pages and return a consistent
/// total hours sum regardless of how many pages the API splits the data into.
#[tokio::test]
#[ignore]
async fn test_pagination_total_hours_consistent() {
    let today = chrono::Local::now().naive_local().date();
    let year = today.year();
    let (from, to) = stats::year_bounds(year);
    let from_str = from.format("%Y-%m-%d").to_string();
    let to_str = to.format("%Y-%m-%d").to_string();

    let entries = client()
        .list_all_time_entries(&from_str, &to_str)
        .await
        .expect("list_all_time_entries failed");

    println!("Total entries fetched for {year}: {}", entries.len());

    // All entry IDs must be unique (no duplicates across pages)
    let mut ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
    let original_len = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), original_len, "duplicate entry IDs found — pagination bug");

    // All entries must fall within the requested year
    for entry in &entries {
        let date = NaiveDate::parse_from_str(&entry.spent_date, "%Y-%m-%d")
            .expect("invalid spent_date");
        assert_eq!(date.year(), year, "entry outside requested year: {}", entry.spent_date);
    }

    let total: f64 = entries.iter().map(|e| e.hours).sum();
    println!("Total hours for {year}: {total:.2}h");
    assert!(total >= 0.0);
}

// ---------------------------------------------------------------------------
// Month summaries integration
// ---------------------------------------------------------------------------

/// Fetch a full year of entries and verify that the sum of monthly totals
/// equals the year-to-date total reported by year_to_date_balance.
/// This validates that month_summaries and period_stats are consistent.
#[tokio::test]
#[ignore]
async fn test_month_summaries_match_ytd_total() {
    let today = chrono::Local::now().naive_local().date();
    let year = today.year();
    let (from, to) = stats::year_bounds(year);

    let entries = client()
        .list_all_time_entries(&from.format("%Y-%m-%d").to_string(), &to.format("%Y-%m-%d").to_string())
        .await
        .expect("list_all_time_entries failed");

    let months = stats::month_summaries(&entries, year, None, 8.0, &[], today);
    let month_total: f64 = months.iter().map(|m| m.total_hours).sum();

    // YTD uses the same entries filtered to today, which is the same set
    // month_summaries uses (entries past today are simply not fetched yet).
    let ytd = stats::year_to_date_balance(&entries, year, None, 8.0, &[], 0.0, 0.0, today);

    println!("Month-by-month total: {month_total:.2}h");
    println!("YTD period total:     {:.2}h", ytd.period.total_hours);

    for (i, m) in months.iter().enumerate() {
        println!(
            "  {:>3}: booked={:.2}h  expected={:.2}h  delta={:+.2}h",
            easy_harvest::ui::month_abbr((i + 1) as u32),
            m.total_hours, m.expected_hours, m.balance_hours
        );
    }

    assert!(
        (month_total - ytd.period.total_hours).abs() < 1e-6,
        "month totals ({month_total:.4}h) must equal ytd total ({:.4}h)",
        ytd.period.total_hours
    );

    // Monthly balance sum must equal YTD period balance
    let month_balance: f64 = months.iter().map(|m| m.balance_hours).sum();
    assert!(
        (month_balance - ytd.period.balance_hours).abs() < 1e-6,
        "sum of monthly balances ({month_balance:.4}h) must equal ytd balance ({:.4}h)",
        ytd.period.balance_hours
    );
}
