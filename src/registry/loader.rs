use anyhow::{Context, Result};
use tokio::fs;

use crate::core::config::Repository;

pub struct RegistryLoader;

impl RegistryLoader {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, repo: &Repository) -> Result<Vec<u8>> {
        let path = repo.get_path();
        let content = fs::read(path)
            .await
            .context("Failed to load registry path.")?;
        Ok(content)
    }
}
