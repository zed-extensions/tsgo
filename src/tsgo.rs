use std::fs;

use zed_extension_api::{self as zed, Result, settings::LspSettings};

struct TsGoExtension {
    server_path: String,
    package_name: String,
    cached_binary_path: Option<String>,
}

impl TsGoExtension {
    fn server_exists(&self) -> bool {
        fs::metadata(self.server_path.clone()).map_or(false, |stat| stat.is_file())
    }
    fn binary_path(&mut self, id: &zed::LanguageServerId) -> Result<String> {
        let server_exists = self.server_exists();
        if self.cached_binary_path.is_some() && server_exists {
            return Ok(self.server_path.clone());
        }

        zed::set_language_server_installation_status(
            id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let version = zed::npm_package_latest_version(&self.package_name)?;

        if !server_exists
            || zed::npm_package_installed_version(&self.package_name)?.as_ref() != Some(&version)
        {
            zed::set_language_server_installation_status(
                id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );
            let result = zed::npm_install_package(&self.package_name, &version);
            match result {
                Ok(()) => {
                    if !self.server_exists() {
                        Err(format!(
                            "installed package '{}' did not contain expected path '{}'",
                            &self.package_name, &self.server_path
                        ))?;
                    }
                }
                Err(error) => {
                    if !self.server_exists() {
                        Err(error)?;
                    }
                }
            }
        }
        Ok(self.server_path.clone())
    }
}

impl zed::Extension for TsGoExtension {
    fn new() -> Self {
        let (platform, arch) = zed::current_platform();

        let platform = match platform {
            zed::Os::Linux => "linux",
            zed::Os::Mac => "darwin",
            zed::Os::Windows => "win32",
        };

        let arch = match arch {
            zed::Architecture::Aarch64 => "arm64",
            zed::Architecture::X86 => todo!(),
            zed::Architecture::X8664 => "x64",
        };

        let package_name = format!("@typescript/native-preview-{platform}-{arch}");
        let server_path = format!(
            "node_modules/{package_name}/lib/tsgo{}",
            if platform == "win32" { ".exe" } else { "" }
        );

        Self {
            server_path,
            package_name,
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
