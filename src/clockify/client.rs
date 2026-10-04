//! Clockify REST API client and interface trait.

use super::models::{
    ClockifyProject, ClockifyTag, ClockifyUser, ClockifyWorkspace, CreateTimeEntryRequest,
    TimeEntryResponse,
};
use super::ClockifyError;

/// Default base URL for Clockify v1 REST API.
pub const DEFAULT_CLOCKIFY_BASE_URL: &str = "https://api.clockify.me/api/v1";

/// Trait defining Clockify REST API operations, facilitating unit testing and mocking.
pub trait ClockifyApi {
    /// Retrieves user information for the authenticated API key.
    fn get_user(&self) -> Result<ClockifyUser, ClockifyError> {
        Err(ClockifyError::Other("Not implemented".to_string()))
    }

    /// Fetches all workspaces accessible to the authenticated user.
    fn get_workspaces(&self) -> Result<Vec<ClockifyWorkspace>, ClockifyError>;

    /// Fetches all projects in the specified workspace.
    fn get_projects(&self, workspace_id: &str) -> Result<Vec<ClockifyProject>, ClockifyError>;

    /// Fetches all tags in the specified workspace.
    fn get_tags(&self, workspace_id: &str) -> Result<Vec<ClockifyTag>, ClockifyError>;

    /// Creates a new time entry in the specified workspace.
    fn create_time_entry(
        &self,
        workspace_id: &str,
        req: &CreateTimeEntryRequest,
    ) -> Result<TimeEntryResponse, ClockifyError>;
}

/// HTTP client for interacting with Clockify REST API.
pub struct ClockifyClient {
    api_key: String,
    base_url: String,
    #[cfg(feature = "cloud-sync")]
    http: reqwest::blocking::Client,
}

impl ClockifyClient {
    /// Creates a new `ClockifyClient` with the default Clockify API endpoint.
    pub fn new(api_key: impl Into<String>) -> Result<Self, ClockifyError> {
        Self::with_base_url(api_key, DEFAULT_CLOCKIFY_BASE_URL)
    }

    /// Creates a new `ClockifyClient` with a custom base URL.
    pub fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, ClockifyError> {
        let key = api_key.into();
        if key.trim().is_empty() {
            return Err(ClockifyError::MissingApiKey);
        }

        let mut url = base_url.into().trim().to_string();
        if url.ends_with('/') {
            url.pop();
        }

        #[cfg(feature = "cloud-sync")]
        {
            let http = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| ClockifyError::Http(e.to_string()))?;

            Ok(Self {
                api_key: key,
                base_url: url,
                http,
            })
        }

        #[cfg(not(feature = "cloud-sync"))]
        {
            Ok(Self {
                api_key: key,
                base_url: url,
            })
        }
    }

    /// Returns the configured API key.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Returns the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[cfg(feature = "cloud-sync")]
    fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, ClockifyError> {
        let resp = self
            .http
            .get(url)
            .header("X-Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .send()
            .map_err(|e| ClockifyError::Http(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            resp.json::<T>()
                .map_err(|e| ClockifyError::Serialization(e.to_string()))
        } else {
            let msg = resp.text().unwrap_or_default();
            match status.as_u16() {
                401 => Err(ClockifyError::Unauthorized),
                404 => Err(ClockifyError::NotFound(msg)),
                code => Err(ClockifyError::ApiError {
                    status: code,
                    message: msg,
                }),
            }
        }
    }

    #[cfg(feature = "cloud-sync")]
    fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T, ClockifyError> {
        let resp = self
            .http
            .post(url)
            .header("X-Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .map_err(|e| ClockifyError::Http(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            resp.json::<T>()
                .map_err(|e| ClockifyError::Serialization(e.to_string()))
        } else {
            let msg = resp.text().unwrap_or_default();
            match status.as_u16() {
                401 => Err(ClockifyError::Unauthorized),
                404 => Err(ClockifyError::NotFound(msg)),
                code => Err(ClockifyError::ApiError {
                    status: code,
                    message: msg,
                }),
            }
        }
    }
}

impl ClockifyApi for ClockifyClient {
    fn get_user(&self) -> Result<ClockifyUser, ClockifyError> {
        #[cfg(feature = "cloud-sync")]
        {
            let url = format!("{}/user", self.base_url);
            self.get(&url)
        }
        #[cfg(not(feature = "cloud-sync"))]
        {
            Err(ClockifyError::Other(
                "cloud-sync feature is disabled".to_string(),
            ))
        }
    }

    fn get_workspaces(&self) -> Result<Vec<ClockifyWorkspace>, ClockifyError> {
        #[cfg(feature = "cloud-sync")]
        {
            let url = format!("{}/workspaces", self.base_url);
            self.get(&url)
        }
        #[cfg(not(feature = "cloud-sync"))]
        {
            Err(ClockifyError::Other(
                "cloud-sync feature is disabled".to_string(),
            ))
        }
    }

    fn get_projects(&self, workspace_id: &str) -> Result<Vec<ClockifyProject>, ClockifyError> {
        if workspace_id.trim().is_empty() {
            return Err(ClockifyError::MissingWorkspace);
        }

        #[cfg(feature = "cloud-sync")]
        {
            let url = format!(
                "{}/workspaces/{workspace_id}/projects?page-size=5000",
                self.base_url
            );
            self.get(&url)
        }
        #[cfg(not(feature = "cloud-sync"))]
        {
            Err(ClockifyError::Other(
                "cloud-sync feature is disabled".to_string(),
            ))
        }
    }

    fn get_tags(&self, workspace_id: &str) -> Result<Vec<ClockifyTag>, ClockifyError> {
        if workspace_id.trim().is_empty() {
            return Err(ClockifyError::MissingWorkspace);
        }

        #[cfg(feature = "cloud-sync")]
        {
            let url = format!(
                "{}/workspaces/{workspace_id}/tags?page-size=5000",
                self.base_url
            );
            self.get(&url)
        }
        #[cfg(not(feature = "cloud-sync"))]
        {
            Err(ClockifyError::Other(
                "cloud-sync feature is disabled".to_string(),
            ))
        }
    }

    fn create_time_entry(
        &self,
        workspace_id: &str,
        req: &CreateTimeEntryRequest,
    ) -> Result<TimeEntryResponse, ClockifyError> {
        if workspace_id.trim().is_empty() {
            return Err(ClockifyError::MissingWorkspace);
        }

        #[cfg(feature = "cloud-sync")]
        {
            let url = format!("{}/workspaces/{workspace_id}/time-entries", self.base_url);
            self.post(&url, req)
        }
        #[cfg(not(feature = "cloud-sync"))]
        {
            Err(ClockifyError::Other(
                "cloud-sync feature is disabled".to_string(),
            ))
        }
    }
}
