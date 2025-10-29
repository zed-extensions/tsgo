use std::fs;
use std::path::PathBuf;

use zed_extension_api::{self as zed, LanguageServerId, Result, settings::LspSettings};

struct TsGoExtension {
    cached_binary_path: Option<String>,
    cached_version: Option<String>,
}

const PACKAGE_NAME: &str = "@typescript/native-preview";

#[derive(Debug)]
struct TsGoSettings {
    package_version: Option<String>,
}

impl TsGoSettings {
    fn from_lsp_settings(settings: &LspSettings) -> Self {
        let package_version = settings
            .settings
            .as_ref()
            .and_then(|s| s.get("package_version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Self {
            package_version,
        }
    }
}

impl TsGoExtension {
    fn get_platform_package_name() -> Result<String> {
        let (platform, arch) = zed::current_platform();

        let os = match platform {
            zed::Os::Mac => "darwin",
            zed::Os::Linux => "linux",
            zed::Os::Windows => "win32",
        };

        let arch = match arch {
            zed::Architecture::Aarch64 => "arm64",
            zed::Architecture::X86 => {
                return Err(
                    "32-bit x86 architecture is not supported. Please use a 64-bit system."
                        .to_string(),
                );
            }
            zed::Architecture::X8664 => "x64",
        };

        Ok(format!("@typescript/native-preview-{}-{}", os, arch))
    }

    fn get_native_binary_path() -> Result<PathBuf> {
        let platform_package = Self::get_platform_package_name()?;

        // Try to find the platform-specific package
        let package_path = PathBuf::from("node_modules").join(&platform_package);

        if !package_path.exists() {
            return Err(format!(
                "Platform package {} not found at {}. Make sure the correct platform-specific package is installed.",
                platform_package,
                package_path.display()
            ));
        }

        let (platform, _) = zed::current_platform();
        let binary_name = match platform {
            zed::Os::Windows => "tsgo.exe",
            _ => "tsgo",
        };

        let binary_path = package_path.join("lib").join(binary_name);

        if !binary_path.exists() {
            return Err(format!(
                "Native binary not found at {}. The platform package may be corrupted.",
                binary_path.display()
            ));
        }

        Ok(binary_path)
    }

    fn binary_exists(&self) -> bool {
        Self::get_native_binary_path().is_ok()
    }

    fn get_installed_version(&self) -> Option<String> {
        zed::npm_package_installed_version(PACKAGE_NAME)
            .ok()
            .flatten()
    }

    fn should_install_or_update(&self, target_version: &str) -> bool {
        if !self.binary_exists() {
            return true;
        }

        match self.get_installed_version() {
            Some(installed_version) => installed_version != target_version,
            None => true,
        }
    }

    fn install_package(&mut self, id: &LanguageServerId, custom_version: Option<&str>) -> Result<()> {
        zed::set_language_server_installation_status(
            id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let target_version = match custom_version {
            Some(version) => version.to_string(),
            None => zed::npm_package_latest_version(PACKAGE_NAME)?,
        };

        if self.should_install_or_update(&target_version) {
            zed::set_language_server_installation_status(
                id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            let result = zed::npm_install_package(PACKAGE_NAME, &target_version);
            if let Err(error) = result {
                if !self.binary_exists() {
                    return Err(error);
                }
            }
        }

        let binary_path = Self::get_native_binary_path()
            .map_err(|e| format!("Failed to locate native binary after installation: {}", e))?;

        // Cache the successful installation
        self.cached_binary_path = Some(binary_path.to_string_lossy().to_string());
        self.cached_version = Some(target_version);

        Ok(())
    }

    fn binary_path(&mut self, id: &LanguageServerId, package_version: Option<&str>) -> Result<String> {
        // Return cached path if we have it and binary still exists
        if let Some(ref cached_path) = self.cached_binary_path {
            if fs::metadata(cached_path).map_or(false, |stat| stat.is_file()) {
                return Ok(cached_path.clone());
            }
        }

        // Install or update package as needed
        self.install_package(id, package_version)?;

        let binary_path = Self::get_native_binary_path()
            .map_err(|e| format!("Failed to locate native binary: {}", e))?;

        Ok(binary_path.to_string_lossy().to_string())
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
        let lsp_settings = LspSettings::for_worktree("tsgo", worktree).ok();
        
        let env = lsp_settings
            .as_ref()
            .and_then(|s| s.binary.as_ref())
            .and_then(|binary| binary.env.clone());

        let settings = lsp_settings
            .as_ref()
            .map(|s| TsGoSettings::from_lsp_settings(s))
            .unwrap_or(TsGoSettings {
                package_version: None,
            });

        let package_version = settings.package_version.as_deref();
        let executable_path = self.binary_path(language_server_id, package_version)?;

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
