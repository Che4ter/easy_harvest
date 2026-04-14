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

use easy_harvest::harvest::client::HarvestClient;
use easy_harvest::harvest::models::{CreateTimeEntry, UpdateTimeEntry};
use easy_harvest::state::settings::Settings;
use easy_harvest::stats;
use chrono::Datelike;

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

    // Use next Monday so this test never touches real bookings on worked days.
    let test_date = "2026-04-13";

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
        .list_all_time_entries(test_date, test_date)
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
    let ytd = stats::year_to_date_balance(&entries, year, None, 8.0, &[], 0.0, today);
    println!("\nYear-to-date balance:");
    println!("  Total hours:    {:.2}h", ytd.period.total_hours);
    println!("  Expected hours: {:.2}h", ytd.period.expected_hours);
    println!("  Period balance: {:+.2}h", ytd.period.balance_hours);
    println!("  Carryover:      {:+.2}h", ytd.carryover_hours);
    println!("  Total balance:  {:+.2}h", ytd.total_balance);
}
