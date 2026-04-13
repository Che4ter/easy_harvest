use reqwest::Client;

use super::models::*;

#[derive(Debug, thiserror::Error)]
pub enum HarvestError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentication failed. Please check your API token and Account ID.")]
    Unauthorized,
    #[error("API error ({status}): {body}")]
    Api { status: u16, body: String },
    #[error("Rate limited — retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
}

#[derive(Clone)]
pub struct HarvestClient {
    http: Client,
    token: String,
    account_id: String,
}

impl HarvestClient {
    pub fn new(token: String, account_id: String) -> Result<Self, reqwest::Error> {
        Ok(Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            token,
            account_id,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("https://api.harvestapp.com/v2{path}"))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Harvest-Account-Id", &self.account_id)
            .header("User-Agent", concat!("EasyHarvest/", env!("CARGO_PKG_VERSION")))
    }

    async fn check_response(
        response: reqwest::Response,
    ) -> Result<reqwest::Response, HarvestError> {
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(HarvestError::Unauthorized);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5);
            return Err(HarvestError::RateLimited {
                retry_after_secs: retry_after,
            });
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(HarvestError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response)
    }

    /// Send a request with automatic retry on 429 (rate limit).
    /// Retries up to 3 times with exponential backoff.
    async fn send_with_retry(
        &self,
        build: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, HarvestError> {
        const MAX_RETRIES: u32 = 3;
        for attempt in 0..=MAX_RETRIES {
            let resp = build().send().await?;
            match Self::check_response(resp).await {
                Ok(resp) => return Ok(resp),
                Err(HarvestError::RateLimited { retry_after_secs }) if attempt < MAX_RETRIES => {
                    tokio::time::sleep(std::time::Duration::from_secs(retry_after_secs)).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    // --- User ---

    pub async fn get_current_user(&self) -> Result<User, HarvestError> {
        let resp = self
            .send_with_retry(|| self.request(reqwest::Method::GET, "/users/me"))
            .await?;
        Ok(resp.json().await?)
    }

    // --- Time Entries ---

    async fn list_time_entries(
        &self,
        from: Option<&str>,
        to: Option<&str>,
        page: Option<i64>,
    ) -> Result<TimeEntriesResponse, HarvestError> {
        let resp = self
            .send_with_retry(|| {
                let mut req = self.request(reqwest::Method::GET, "/time_entries");
                if let Some(from) = from {
                    req = req.query(&[("from", from)]);
                }
                if let Some(to) = to {
                    req = req.query(&[("to", to)]);
                }
                req = req.query(&[("per_page", "2000")]);
                if let Some(page) = page {
                    req = req.query(&[("page", &page.to_string())]);
                }
                req
            })
            .await?;
        Ok(resp.json().await?)
    }

    /// Fetch all time entries for a date range, handling pagination.
    pub async fn list_all_time_entries(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<TimeEntry>, HarvestError> {
        let mut all_entries = Vec::new();
        let mut page = 1;
        loop {
            let resp = self.list_time_entries(Some(from), Some(to), Some(page)).await?;
            all_entries.extend(resp.time_entries);
            if page >= resp.total_pages {
                break;
            }
            page += 1;
        }
        Ok(all_entries)
    }

    pub async fn create_time_entry(
        &self,
        entry: &CreateTimeEntry,
    ) -> Result<TimeEntry, HarvestError> {
        let resp = self
            .send_with_retry(|| {
                self.request(reqwest::Method::POST, "/time_entries")
                    .json(entry)
            })
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn update_time_entry(
        &self,
        id: i64,
        entry: &UpdateTimeEntry,
    ) -> Result<TimeEntry, HarvestError> {
        let resp = self
            .send_with_retry(|| {
                self.request(reqwest::Method::PATCH, &format!("/time_entries/{id}"))
                    .json(entry)
            })
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn delete_time_entry(&self, id: i64) -> Result<(), HarvestError> {
        self.send_with_retry(|| {
            self.request(reqwest::Method::DELETE, &format!("/time_entries/{id}"))
        })
        .await?;
        Ok(())
    }

    /// Start (restart) the timer on an existing time entry.
    /// Harvest allows only one running timer at a time; starting a second one stops the first.
    pub async fn restart_timer(&self, id: i64) -> Result<TimeEntry, HarvestError> {
        let resp = self
            .send_with_retry(|| {
                self.request(reqwest::Method::PATCH, &format!("/time_entries/{id}/restart"))
            })
            .await?;
        Ok(resp.json().await?)
    }

    /// Stop the running timer on a time entry.
    pub async fn stop_timer(&self, id: i64) -> Result<TimeEntry, HarvestError> {
        let resp = self
            .send_with_retry(|| {
                self.request(reqwest::Method::PATCH, &format!("/time_entries/{id}/stop"))
            })
            .await?;
        Ok(resp.json().await?)
    }

    // --- Project Assignments (projects assigned to current user) ---

    async fn list_my_project_assignments(
        &self,
        page: Option<i64>,
    ) -> Result<ProjectAssignmentsResponse, HarvestError> {
        let resp = self
            .send_with_retry(|| {
                let mut req =
                    self.request(reqwest::Method::GET, "/users/me/project_assignments");
                req = req.query(&[("per_page", "2000")]);
                if let Some(page) = page {
                    req = req.query(&[("page", &page.to_string())]);
                }
                req
            })
            .await?;
        Ok(resp.json().await?)
    }

    /// Fetch all project assignments for the current user, handling pagination.
    pub async fn list_all_my_project_assignments(
        &self,
    ) -> Result<Vec<ProjectAssignment>, HarvestError> {
        let mut all = Vec::new();
        let mut page = 1;
        loop {
            let resp = self.list_my_project_assignments(Some(page)).await?;
            all.extend(resp.project_assignments);
            if page >= resp.total_pages {
                break;
            }
            page += 1;
        }
        Ok(all)
    }
}
