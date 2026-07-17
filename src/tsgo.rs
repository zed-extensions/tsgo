mod settings;
mod typescript_package;

use settings::{ExtensionSetting, LspSettingsWithFallback};
use zed_extension_api::{self as zed, LanguageServerId, Result};

struct TsGoExtension {
    installed_spec: Option<String>,
}

impl TsGoExtension {
    /// Resolves the TypeScript 7+ package to run, preferring an explicit
    /// `tsdk.path`, then a project-local dependency, then a managed install.
    fn resolve_package_dir(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
        extension_settings: &Option<zed::serde_json::Value>,
    ) -> Result<String> {
        if let Some(tsdk_path) =
            settings::string_setting(extension_settings, ExtensionSetting::TsdkPath)?
        {
            let directory = typescript_package::tsdk_package_dir(worktree, &tsdk_path);
            if let Some(version) =
                typescript_package::typescript_version_from_package_dir(worktree, &directory)
                    .map_err(|error| {
                        format!("tsdk.path `{tsdk_path}` resolved to `{directory}`: {error}")
                    })?
            {
                typescript_package::ensure_typescript_7_or_newer(&version)?;
            }
            return Ok(directory);
        }

        if let Some(directory) = typescript_package::find_local_typescript_package_dir(worktree) {
            return Ok(directory);
        }

        self.install_managed(language_server_id, extension_settings)
    }

    fn install_managed(
        &mut self,
        language_server_id: &LanguageServerId,
        extension_settings: &Option<zed::serde_json::Value>,
    ) -> Result<String> {
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let requested = typescript_package::requested_typescript_spec(extension_settings)?;

        if self.installed_spec.as_deref() == Some(requested.install_spec.as_str())
            && typescript_package::managed_package_is_usable()
        {
            return typescript_package::managed_package_dir();
        }

        let package_directory =
            typescript_package::install_managed_typescript(language_server_id, &requested)?;
        self.installed_spec = Some(requested.install_spec);
        Ok(package_directory)
    }

    fn build_language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let lsp_settings = LspSettingsWithFallback::for_worktree(language_server_id, worktree).ok();
        let extension_settings = lsp_settings
            .as_ref()
            .and_then(|settings| settings.get_setting(|settings| settings.settings.as_ref()))
            .cloned();
        let binary_env = lsp_settings
            .and_then(|settings| settings.into_setting(|settings| settings.binary))
            .and_then(|binary| binary.env);

        let package_directory =
            self.resolve_package_dir(language_server_id, worktree, &extension_settings)?;
        let arguments = vec!["--lsp".into(), "--stdio".into()];
        let env = server_environment(worktree, binary_env);

        // Prefer the native executable directly when the package layout can be
        // resolved without Node.
        if let Some(native) =
            typescript_package::find_native_server_binary(worktree, &package_directory)
        {
            return Ok(zed::Command {
                command: native,
                args: arguments,
                env,
            });
        }

        // Microsoft's Node launcher handles pnpm and other non-hoisted layouts.
        let node_command = if let Some(path) = worktree.which("node") {
            path
        } else {
            zed::node_binary_path()?
        };
        let shim = typescript_package::node_shim_path(worktree, &package_directory)?;
        let arguments = std::iter::once(shim).chain(arguments).collect();

        Ok(zed::Command {
            command: node_command,
            args: arguments,
            env,
        })
    }
}

impl zed::Extension for TsGoExtension {
    fn new() -> Self {
        Self {
            installed_spec: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
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
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(
            LspSettingsWithFallback::for_worktree(language_server_id, worktree)?
                .into_setting(|settings| settings.initialization_options),
        )
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        let settings = LspSettingsWithFallback::for_worktree(language_server_id, worktree)?
            .into_setting(|settings| settings.settings)
            .unwrap_or_default();
        Ok(Some(settings))
    }
}

fn server_environment(
    worktree: &zed::Worktree,
    binary_env: Option<std::collections::HashMap<String, String>>,
) -> Vec<(String, String)> {
    let mut env = worktree.shell_env();

    if let Some(binary_env) = binary_env {
        for (key, value) in binary_env {
            upsert_environment_variable(&mut env, key, value);
        }
    }

    env
}

fn upsert_environment_variable(env: &mut Vec<(String, String)>, key: String, value: String) {
    env.retain(|(existing_key, _)| *existing_key != key);
    env.push((key, value));
}

zed::register_extension!(TsGoExtension);
