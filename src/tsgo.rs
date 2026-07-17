mod typescript_package;

use std::cell::OnceCell;
use std::fs;
use std::path::PathBuf;

use zed_extension_api::serde_json::Value;
use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree, settings::LspSettings};

struct TsGoExtension {
    cached_binary_path: Option<String>,
    cached_version: Option<String>,
}

const PACKAGE_NAME: &str = "typescript";
/// LSP ID previously used before TypeScript 7 was released
const FALLBACK_KEY: &str = "tsgo";

#[derive(Debug, Default)]
struct TsGoSettings {
    package_version: Option<String>,
}

impl TsGoSettings {
    fn from_lsp_settings(settings: &Value) -> Self {
        let package_version = settings
            .get("package_version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Self { package_version }
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

        Ok(format!("@typescript/typescript-{}-{}", os, arch))
    }

    fn get_native_binary_path() -> Result<PathBuf> {
        let platform_package = Self::get_platform_package_name()?;

        // Try to find the platform-specific package
        let package_path = PathBuf::from("node_modules").join(&platform_package);

        if !package_path.exists() {
            return Err(format!(
                "Platform package {platform_package} not found at {package_path}. \
                Make sure the correct platform-specific package is installed \
                (a pinned package_version must be >= 7.0.0; older typescript versions have no native binary).",
                package_path = package_path.display()
            ));
        }

        let (platform, _) = zed::current_platform();
        let binary_name = match platform {
            zed::Os::Windows => "tsc.exe",
            _ => "tsc",
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

    fn install_package(
        &mut self,
        id: &LanguageServerId,
        custom_version: Option<&str>,
    ) -> Result<()> {
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
            if let Err(error) = result
                && !self.binary_exists()
            {
                return Err(error);
            }
        }

        let binary_path = Self::get_native_binary_path()
            .map_err(|e| format!("Failed to locate native binary after installation: {}", e))?;

        // Cache the successful installation
        self.cached_binary_path = Some(binary_path.to_string_lossy().to_string());
        self.cached_version = Some(target_version);

        Ok(())
    }

    fn binary_path(
        &mut self,
        id: &LanguageServerId,
        package_version: Option<&str>,
    ) -> Result<String> {
        // Return cached path if we have it and binary still exists
        if let Some(cached_path) = self.cached_binary_path.as_ref()
            && fs::metadata(cached_path).is_ok_and(|stat| stat.is_file())
        {
            return Ok(cached_path.clone());
        }

        // Install or update package as needed
        self.install_package(id, package_version)?;

        let binary_path = Self::get_native_binary_path()
            .map_err(|e| format!("Failed to locate native binary: {}", e))?;

        Ok(binary_path.to_string_lossy().to_string())
    }

    fn build_language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        let lsp_settings =
            LspSettingsWithFallback::for_worktree(language_server_id, FALLBACK_KEY, worktree).ok();

        let settings = lsp_settings
            .as_ref()
            .and_then(|settings| {
                settings
                    .get_setting(|settings| settings.settings.as_ref())
                    .map(TsGoSettings::from_lsp_settings)
            })
            .unwrap_or_default();

        let binary_env = lsp_settings
            .and_then(|settings| settings.into_setting(|settings| settings.binary))
            .and_then(|binary| binary.env);
        let env = server_environment(worktree, binary_env);
        let arguments = vec!["--lsp".into(), "--stdio".into()];

        if let Some(package_directory) =
            typescript_package::find_local_typescript_package_dir(worktree)
        {
            if let Some(native) =
                typescript_package::find_native_server_binary(worktree, &package_directory)
            {
                return Ok(zed::Command {
                    command: native,
                    args: arguments,
                    env,
                });
            }

            let node = worktree
                .which("node")
                .map(Ok)
                .unwrap_or_else(zed::node_binary_path)?;
            let shim = typescript_package::node_shim_path(worktree, &package_directory)?;
            return Ok(zed::Command {
                command: node,
                args: std::iter::once(shim).chain(arguments).collect(),
                env,
            });
        }

        let executable_path =
            self.binary_path(language_server_id, settings.package_version.as_deref())?;
        Ok(zed::Command {
            command: std::env::current_dir()
                .map_err(|error| error.to_string())?
                .join(executable_path)
                .to_string_lossy()
                .into_owned(),
            args: arguments,
            env,
        })
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
        match self.build_language_server_command(language_server_id, worktree) {
            Ok(command) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                Ok(command)
            }
            Err(error) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::Failed(error.clone()),
                );
                Err(error)
            }
        }
    }

    fn language_server_initialization_options(
        &mut self,
        server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<Option<zed_extension_api::serde_json::Value>> {
        let settings = LspSettingsWithFallback::for_worktree(server_id, FALLBACK_KEY, worktree)?
            .into_setting(|lsp_settings| lsp_settings.initialization_options)
            .unwrap_or_default();
        Ok(Some(settings))
    }

    fn language_server_workspace_configuration(
        &mut self,
        server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<Option<zed_extension_api::serde_json::Value>> {
        let settings = LspSettingsWithFallback::for_worktree(server_id, FALLBACK_KEY, worktree)?
            .into_setting(|lsp_settings| lsp_settings.settings)
            .unwrap_or_default();
        Ok(Some(settings))
    }
}

fn server_environment(
    worktree: &Worktree,
    binary_env: Option<std::collections::HashMap<String, String>>,
) -> Vec<(String, String)> {
    let mut env = worktree.shell_env();
    if let Some(binary_env) = binary_env {
        for (key, value) in binary_env {
            env.retain(|(existing_key, _)| *existing_key != key);
            env.push((key, value));
        }
    }
    env
}

struct LspSettingsWithFallback<'a> {
    settings: LspSettings,
    fallback_id: &'static str,
    fallback_settings: OnceCell<Option<LspSettings>>,
    worktree: &'a Worktree,
}

impl<'a> LspSettingsWithFallback<'a> {
    fn for_worktree(
        server_id: &LanguageServerId,
        fallback_id: &'static str,
        worktree: &'a Worktree,
    ) -> zed_extension_api::Result<Self> {
        LspSettings::for_worktree(server_id.as_ref(), worktree).map(|settings| Self {
            settings,
            fallback_id,
            fallback_settings: OnceCell::new(),
            worktree,
        })
    }

    fn get_setting<F, R>(&self, f: F) -> Option<&R>
    where
        F: Fn(&LspSettings) -> Option<&R>,
    {
        f(&self.settings).or_else(|| {
            self.fallback_settings
                .get_or_init(|| LspSettings::for_worktree(self.fallback_id, self.worktree).ok())
                .as_ref()
                .and_then(f)
        })
    }

    fn into_setting<F, R>(self, f: F) -> Option<R>
    where
        F: Fn(LspSettings) -> Option<R>,
    {
        f(self.settings).or_else(|| {
            let _ = self
                .fallback_settings
                .get_or_init(|| LspSettings::for_worktree(self.fallback_id, self.worktree).ok());
            self.fallback_settings.into_inner().flatten().and_then(f)
        })
    }
}

zed::register_extension!(TsGoExtension);
