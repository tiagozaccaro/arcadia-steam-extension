use arcadia_extension_framework::{
    models::{ExtensionManifest, ExtensionType},
    traits::{ExtensionImpl, ExtensionContext},
    error::ExtensionError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

// Steam-specific data structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamApp {
    pub appid: u32,
    pub name: String,
    pub install_dir: Option<String>,
    pub size_on_disk: Option<u64>,
    pub last_updated: Option<u64>,
    pub launch_options: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub app: SteamApp,
    pub executable: Option<String>,
    pub working_dir: Option<String>,
    pub launch_args: Option<String>,
    pub icon_path: Option<String>,
    pub banner_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamLibrary {
    pub path: PathBuf,
    pub apps: HashMap<u32, SteamApp>,
}

pub struct SteamExtension {
    manifest: ExtensionManifest,
    libraries: Vec<SteamLibrary>,
    steam_install_path: Option<PathBuf>,
}

impl SteamExtension {
    pub fn new() -> Self {
        let manifest = ExtensionManifest {
            name: "Steam Game Library Extension".to_string(),
            version: "0.1.0".to_string(),
            author: Some("Arcadia Team".to_string()),
            description: Some("Extension for integrating Steam game library into Arcadia".to_string()),
            extension_type: ExtensionType::GameLibrary,
            entry_point: "arcadia_steam_extension".to_string(),
            permissions: vec!["filesystem".to_string(), "native".to_string()],
            dependencies: None,
            hooks: Some(vec![
                "scan_games".to_string(),
                "get_game_details".to_string(),
                "launch_game".to_string(),
            ]),
            apis: Some(serde_json::from_str(r#"{"provided": ["steam_games", "steam_launcher"]}"#).unwrap()),
            menu_items: None,
        };

        Self {
            manifest,
            libraries: Vec::new(),
            steam_install_path: None,
        }
    }

    async fn find_steam_install_path(&mut self) -> Result<(), ExtensionError> {
        // Common Steam installation paths
        let possible_paths = if cfg!(target_os = "windows") {
            vec![
                "C:\\Program Files (x86)\\Steam",
                "C:\\Program Files\\Steam",
            ]
        } else if cfg!(target_os = "macos") {
            vec![
                "/Applications/Steam.app/Contents/MacOS",
                "~/Library/Application Support/Steam",
            ]
        } else {
            vec![
                "~/.steam/steam",
                "~/.local/share/Steam",
            ]
        };

        for path_str in possible_paths {
            let path = if path_str.starts_with('~') {
                dirs::home_dir()
                    .ok_or_else(|| ExtensionError::Validation("Could not find home directory".to_string()))?
                    .join(&path_str[2..])
            } else {
                PathBuf::from(path_str)
            };

            if path.exists() {
                self.steam_install_path = Some(path);
                return Ok(());
            }
        }

        Err(ExtensionError::NotFound("Steam installation not found".to_string()))
    }

    async fn scan_steam_libraries(&mut self) -> Result<(), ExtensionError> {
        let steam_path = self.steam_install_path.as_ref()
            .ok_or_else(|| ExtensionError::Validation("Steam path not set".to_string()))?;

        let _config_path = if cfg!(target_os = "windows") {
            steam_path.join("config").join("config.vdf")
        } else {
            steam_path.join("config").join("config.vdf")
        };

        // For simplicity, assume default library path
        let default_library = steam_path.join("steamapps");
        if default_library.exists() {
            let library = SteamLibrary {
                path: default_library,
                apps: HashMap::new(),
            };
            self.libraries.push(library);
        }

        Ok(())
    }

    async fn scan_games_in_library(&self, library_path: &PathBuf) -> Result<HashMap<u32, SteamApp>, ExtensionError> {
        let mut apps = HashMap::new();
        let mut entries = fs::read_dir(library_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("acf") {
                if let Some(app) = self.parse_app_manifest(&path).await? {
                    apps.insert(app.appid, app);
                }
            }
        }
        Ok(apps)
    }

    async fn parse_app_manifest(&self, path: &PathBuf) -> Result<Option<SteamApp>, ExtensionError> {
        let content = fs::read_to_string(path).await?;
        // Simple VDF parsing (Valve Data Format)
        // This is a basic implementation - real VDF parsing would be more complex
        let appid = self.extract_vdf_value(&content, "appid")?;
        let name = self.extract_vdf_value(&content, "name")?;
        let install_dir = self.extract_vdf_value(&content, "installdir").ok();
        let size_on_disk = self.extract_vdf_value(&content, "SizeOnDisk")
            .ok()
            .and_then(|s| s.parse().ok());

        let app = SteamApp {
            appid: appid.parse().map_err(|_| ExtensionError::Validation("Invalid appid".to_string()))?,
            name,
            install_dir,
            size_on_disk,
            last_updated: None,
            launch_options: None,
        };

        Ok(Some(app))
    }

    fn extract_vdf_value(&self, content: &str, key: &str) -> Result<String, ExtensionError> {
        // Very basic VDF extraction - in reality, use a proper VDF parser
        for line in content.lines() {
            let line = line.trim();
            if line.contains(&format!("\"{}\"", key)) {
                if let Some(start) = line.find(&format!("\"{}\"", key)) {
                    let after_key = &line[start + key.len() + 2..];
                    if let Some(quote_start) = after_key.find('"') {
                        let after_quote = &after_key[quote_start + 1..];
                        if let Some(quote_end) = after_quote.find('"') {
                            return Ok(after_quote[..quote_end].to_string());
                        }
                    }
                }
            }
        }
        Err(ExtensionError::Validation(format!("Key {} not found", key)))
    }

    async fn get_game_details(&self, appid: u32) -> Result<SteamGame, ExtensionError> {
        for library in &self.libraries {
            if let Some(app) = library.apps.get(&appid) {
                let game_dir = library.path.join("common").join(app.install_dir.as_ref().unwrap_or(&"".to_string()));
                let executable = self.find_executable(&game_dir).await?;
                let icon_path = self.find_icon(appid).await.ok();

                let game = SteamGame {
                    app: app.clone(),
                    executable,
                    working_dir: Some(game_dir.to_string_lossy().to_string()),
                    launch_args: None,
                    icon_path: icon_path.unwrap_or(None),
                    banner_path: None,
                };

                return Ok(game);
            }
        }
        Err(ExtensionError::NotFound(format!("Game with appid {} not found", appid)))
    }

    async fn find_executable(&self, game_dir: &PathBuf) -> Result<Option<String>, ExtensionError> {
        // Simple executable finding - look for common executable names
        let common_executables = ["game.exe", "Game.exe", "launch.exe", "start.exe"];

        for exe in &common_executables {
            let exe_path = game_dir.join(exe);
            if exe_path.exists() {
                return Ok(Some(exe_path.to_string_lossy().to_string()));
            }
        }

        // If no common executable found, look for any .exe file
        if cfg!(target_os = "windows") {
            let mut entries = fs::read_dir(game_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(ext) = entry.path().extension() {
                    if ext == "exe" {
                        return Ok(Some(entry.path().to_string_lossy().to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn find_icon(&self, appid: u32) -> Result<Option<String>, ExtensionError> {
        if let Some(steam_path) = &self.steam_install_path {
            let icon_path = steam_path.join("appcache").join("librarycache").join(format!("{}_icon.jpg", appid));
            if icon_path.exists() {
                return Ok(Some(icon_path.to_string_lossy().to_string()));
            }
        }
        Ok(None)
    }

    async fn launch_game(&self, appid: u32) -> Result<(), ExtensionError> {
        let game = self.get_game_details(appid).await?;
        if let Some(executable) = game.executable {
            // Use std::process::Command to launch the game
            std::process::Command::new(&executable)
                .current_dir(game.working_dir.as_ref().unwrap_or(&".".to_string()))
                .spawn()
                .map_err(|e| ExtensionError::Io(e))?;
            Ok(())
        } else {
            Err(ExtensionError::Validation("No executable found for game".to_string()))
        }
    }
}

#[async_trait]
impl ExtensionImpl for SteamExtension {
    async fn initialize(&mut self, _context: &ExtensionContext) -> Result<(), ExtensionError> {
        self.find_steam_install_path().await?;
        self.scan_steam_libraries().await?;
        let paths: Vec<PathBuf> = self.libraries.iter().map(|l| l.path.clone()).collect();
        for (i, path) in paths.into_iter().enumerate() {
            let apps = self.scan_games_in_library(&path).await?;
            self.libraries[i].apps = apps;
        }
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), ExtensionError> {
        // Cleanup if needed
        Ok(())
    }

    async fn handle_hook(&self, hook: &str, params: Value) -> Result<Value, ExtensionError> {
        match hook {
            "scan_games" => {
                let games: Vec<SteamGame> = self.libraries.iter()
                    .flat_map(|lib| lib.apps.values())
                    .map(|app| SteamGame {
                        app: app.clone(),
                        executable: None,
                        working_dir: None,
                        launch_args: None,
                        icon_path: None,
                        banner_path: None,
                    })
                    .collect();
                Ok(serde_json::to_value(games)?)
            }
            "get_game_details" => {
                let appid = params.get("appid")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ExtensionError::Validation("appid parameter required".to_string()))?;
                let game = self.get_game_details(appid as u32).await?;
                Ok(serde_json::to_value(game)?)
            }
            "launch_game" => {
                let appid = params.get("appid")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ExtensionError::Validation("appid parameter required".to_string()))?;
                self.launch_game(appid as u32).await?;
                Ok(Value::Null)
            }
            _ => Err(ExtensionError::Validation(format!("Unknown hook: {}", hook))),
        }
    }

    fn get_manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn get_type(&self) -> ExtensionType {
        ExtensionType::GameLibrary
    }

    fn get_id(&self) -> &str {
        "steam_extension"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let extension = SteamExtension::new();
        assert_eq!(extension.get_id(), "steam_extension");
    }
}
