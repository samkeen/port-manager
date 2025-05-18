use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, Context};
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// Minimum port to display (inclusive)
    pub min_port: u16,
    /// Maximum port to display (inclusive)
    pub max_port: u16,
    /// List of process names to filter out
    pub filtered_process_names: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Default to non-privileged ports (above 1023)
            min_port: 1024,
            // Common maximum for ephemeral ports
            max_port: 49151,
            // Default filtered process names
            filtered_process_names: vec![
                "Browser".to_string(),
                "ControlCE".to_string(),
            ],
        }
    }
}

impl Config {
    /// Get the config file path
    fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "portmanager", "portmanager")
            .context("Could not determine config directory")?;
        
        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir).context("Failed to create config directory")?;
        
        Ok(config_dir.join("config.json"))
    }
    
    /// Load configuration from disk, or create default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            let config_str = fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            
            serde_json::from_str(&config_str)
                .context("Failed to parse config file")
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }
    
    /// Save configuration to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let config_str = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(&config_path, config_str)
            .context("Failed to write config file")?;
        
        Ok(())
    }
    
    /// Add a process name to the filter list
    pub fn add_filtered_process(&mut self, process_name: String) -> Result<()> {
        if !self.filtered_process_names.contains(&process_name) {
            self.filtered_process_names.push(process_name);
            self.save()?;
        }
        Ok(())
    }
    
    /// Remove a process name from the filter list
    pub fn remove_filtered_process(&mut self, process_name: &str) -> Result<()> {
        self.filtered_process_names.retain(|name| name != process_name);
        self.save()
    }
}
