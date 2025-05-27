use std::fs;

use zed_extension_api::{self as zed, LanguageServerId, Result, settings::LspSettings};

struct TsGoExtension {
    cached_binary_path: Option<String>,
    cached_version: Option<String>,
}

const PACKAGE_NAME: &str = "@typescript/native-preview";
const BIN_PATH: &str = "node_modules/.bin/tsgo";

impl TsGoExtension {
    fn binary_exists(&self) -> bool {
        fs::metadata(BIN_PATH).map_or(false, |stat| stat.is_file())
    }

    fn get_installed_version(&self) -> Option<String> {
        zed::npm_package_installed_version(PACKAGE_NAME)
            .ok()
            .flatten()
    }

    fn should_install_or_update(&self, latest_version: &str) -> bool {
        if !self.binary_exists() {
            return true;
        }

        match self.get_installed_version() {
            Some(installed_version) => installed_version != latest_version,
            None => true,
        }
    }

    fn install_package(&mut self, id: &LanguageServerId) -> Result<()> {
        zed::set_language_server_installation_status(
            id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let latest_version = zed::npm_package_latest_version(PACKAGE_NAME)?;

        if self.should_install_or_update(&latest_version) {
            zed::set_language_server_installation_status(
                id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            let result = zed::npm_install_package(PACKAGE_NAME, &latest_version);
            if let Err(error) = result {
                if !self.binary_exists() {
                    return Err(error);
                }
            }
        }

        if !self.binary_exists() {
            return Err(format!(
                "installed package did not contain expected binary at '{}'",
                BIN_PATH
            ));
        }

        // Cache the successful installation
        self.cached_binary_path = Some(BIN_PATH.to_string());
        self.cached_version = Some(latest_version);

        Ok(())
    }

    fn binary_path(&mut self, id: &LanguageServerId) -> Result<String> {
        // Return cached path if we have it and binary still exists
        if let Some(ref cached_path) = self.cached_binary_path {
            if fs::metadata(cached_path).map_or(false, |stat| stat.is_file()) {
                return Ok(cached_path.clone());
            }
        }

        // Install or update package as needed
        self.install_package(id)?;

        Ok(BIN_PATH.to_string())
    }
}

impl zed::Extension for TsGoExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
            cached_version: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<zed_extension_api::Command> {
        let env = LspSettings::for_worktree("tsgo", worktree)
            .ok()
            .and_then(|s| s.binary)
            .and_then(|binary| binary.env);

        let executable_path = self.binary_path(language_server_id)?;

        Ok(zed::Command {
            command: std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(executable_path)
                .to_string_lossy()
                .into_owned(),
            args: vec!["--lsp".into(), "--stdio".into()],
            env: env.into_iter().flat_map(|env| env.into_iter()).collect(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<Option<zed_extension_api::serde_json::Value>> {
        let settings = LspSettings::for_worktree(server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.initialization_options.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }

    fn language_server_workspace_configuration(
        &mut self,
        server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<Option<zed_extension_api::serde_json::Value>> {
        let settings = LspSettings::for_worktree(server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }
}

zed::register_extension!(TsGoExtension);
