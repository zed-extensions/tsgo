use std::fs;

use zed_extension_api::{self as zed, Result, settings::LspSettings};

struct CurrentPlatform(zed::Os, zed::Architecture);

impl CurrentPlatform {
    pub fn get_package_name_and_server_path(&self) -> Result<(String, String)> {
        let platform = match self.0 {
            zed::Os::Linux => "linux",
            zed::Os::Mac => "darwin",
            zed::Os::Windows => "win32",
        };

        let arch = match self.1 {
            zed::Architecture::Aarch64 => "arm64",
            zed::Architecture::X86 => return Err("32-bit architecture is not supported".into()),
            zed::Architecture::X8664 => "x64",
        };

        let package_name = format!("@typescript/native-preview-{platform}-{arch}");
        let server_path = format!(
            "node_modules/{package_name}/lib/tsgo{}",
            if self.0 == zed::Os::Windows {
                ".exe"
            } else {
                ""
            }
        );

        Ok((package_name, server_path))
    }
}

struct TsGoExtension {
    current_platform: CurrentPlatform,
    cached_binary_path: Option<String>,
}

impl TsGoExtension {
    fn server_exists(&self, server_path: &str) -> bool {
        fs::metadata(server_path).map_or(false, |stat| stat.is_file())
    }

    fn binary_path(&mut self, id: &zed::LanguageServerId) -> Result<String> {
        let (package_name, server_path) =
            self.current_platform.get_package_name_and_server_path()?;
        let server_exists = self.server_exists(&server_path);
        if self.cached_binary_path.is_some() && server_exists {
            return Ok(server_path.clone());
        }

        zed::set_language_server_installation_status(
            id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let version = zed::npm_package_latest_version(&package_name)?;

        if !server_exists
            || zed::npm_package_installed_version(&package_name)?.as_ref() != Some(&version)
        {
            zed::set_language_server_installation_status(
                id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );
            let result = zed::npm_install_package(&package_name, &version);
            match result {
                Ok(()) => {
                    if !self.server_exists(&server_path) {
                        Err(format!(
                            "installed package '{}' did not contain expected path '{}'",
                            &package_name, &server_path
                        ))?;
                    }
                }
                Err(error) => {
                    if !self.server_exists(&server_path) {
                        Err(error)?;
                    }
                }
            }
        }
        Ok(server_path.clone())
    }
}

impl zed::Extension for TsGoExtension {
    fn new() -> Self {
        let (os, arch) = zed::current_platform();

        Self {
            current_platform: CurrentPlatform(os, arch),
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let env = LspSettings::for_worktree("tsgo", worktree)
            .ok()
            .and_then(|s| s.binary)
            .and_then(|binary| binary.env);
        let command = self.binary_path(language_server_id)?;
        self.cached_binary_path = Some(command.clone());
        Ok(zed::Command {
            command: std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(command)
                .to_string_lossy()
                .into_owned(),
            args: vec!["--lsp".into(), "--stdio".into()],
            env: env.into_iter().flat_map(|env| env.into_iter()).collect(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.initialization_options.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }

    fn language_server_workspace_configuration(
        &mut self,
        server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(server_id.as_ref(), worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();
        Ok(Some(settings))
    }
}

zed::register_extension!(TsGoExtension);
