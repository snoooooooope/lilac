use crate::error::{AurError, aur_request_failed, aur_parse_error, aur_api_error};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use serde_json;

#[derive(Debug, Deserialize)]
pub struct AurPackage {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    pub num_votes: u32,
    #[serde(rename = "Popularity")]
    pub popularity: f32,
    #[serde(rename = "FirstSubmitted")]
    pub first_submitted: u64,
    #[serde(rename = "LastModified")]
    pub last_modified: u64,
}

#[derive(Debug, Deserialize)]
struct AurResponse {
    results: Vec<AurPackage>,
}

pub struct AurClient {
    base_url: String,
    client: Client,
}

impl AurClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
            
        AurClient { base_url, client }
    }

    pub async fn search_packages(&self, query: &str) -> Result<Vec<AurPackage>, AurError> {
        let url = format!("{}/rpc/?v=5&type=search&by=name&arg={}", self.base_url, query);

        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| aur_request_failed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(aur_api_error(format!("Status: {}", response.status())));
        }

        let raw_response = response.text().await.map_err(|e| aur_parse_error(e.to_string()))?;
        serde_json::from_str::<AurResponse>(&raw_response)
            .map(|r| r.results)
            .map_err(|e| aur_parse_error(e.to_string()))
    }

    pub async fn get_package_info(&self, package_name: &str) -> Result<AurPackage, AurError> {
        let url = format!("{}/rpc/?v=5&type=info&arg={}", self.base_url, package_name);

        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| aur_request_failed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(aur_api_error(format!("Status: {}", response.status())));
        }

        let mut aur_response: AurResponse = response.json()
            .await
            .map_err(|e| aur_parse_error(e.to_string()))?;

        aur_response.results.pop()
            .ok_or_else(|| AurError::NotFound(package_name.to_string()))
    }
}
