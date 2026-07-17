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
        let binary =
            lsp_settings.and_then(|settings| settings.into_setting(|settings| settings.binary));
        let binary_env = binary.as_ref().and_then(|binary| binary.env.clone());

        // A configured binary may be a direct tsc launcher or a custom Node
        // executable. Explicit arguments always override extension defaults.
        if let Some(path) = binary.as_ref().and_then(|binary| binary.path.clone()) {
            let user_arguments = binary
                .and_then(|binary| binary.arguments)
                .unwrap_or_default();
            let (command, arguments) = if !user_arguments.is_empty() {
                (path, user_arguments)
            } else {
                let normalized = path.replace('\\', "/");
                let is_tsc_launcher = normalized.ends_with("tsc")
                    || normalized.ends_with("tsc.js")
                    || normalized.ends_with("tsc.exe");
                if is_tsc_launcher {
                    (path, language_server_arguments(&extension_settings)?)
                } else {
                    let package_directory = self.resolve_package_dir(
                        language_server_id,
                        worktree,
                        &extension_settings,
                    )?;
                    let shim = typescript_package::node_shim_path(worktree, &package_directory)?;
                    let arguments = std::iter::once(shim)
                        .chain(language_server_arguments(&extension_settings)?)
                        .collect();
                    (path, arguments)
                }
            };
            let env = server_environment(worktree, &extension_settings, binary_env)?;
            return Ok(zed::Command {
                command,
                args: arguments,
                env,
            });
        }

        let package_directory =
            self.resolve_package_dir(language_server_id, worktree, &extension_settings)?;
        let arguments = language_server_arguments(&extension_settings)?;
        let env = server_environment(worktree, &extension_settings, binary_env)?;

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
        let extension_settings =
            LspSettingsWithFallback::for_worktree(language_server_id, worktree)?
                .into_setting(|settings| settings.settings);
        Ok(settings::workspace_configuration(extension_settings))
    }

    fn label_for_completion(
        &self,
        _language_server_id: &LanguageServerId,
        completion: zed::lsp::Completion,
    ) -> Option<zed::CodeLabel> {
        use zed::lsp::CompletionKind as Kind;

        let highlight_name = match completion.kind? {
            Kind::Class | Kind::Interface | Kind::Enum | Kind::Constructor => "type",
            Kind::Constant => "constant",
            Kind::Function | Kind::Method => "function",
            Kind::Property | Kind::Field => "property",
            Kind::Variable => "variable",
            _ => return None,
        };

        let label = completion.label;
        let name_length = label.len();
        let mut code = label.clone();
        let mut spans = vec![zed::CodeLabelSpan::literal(
            label,
            Some(highlight_name.to_string()),
        )];

        if let Some(detail) = completion
            .label_details
            .as_ref()
            .and_then(|details| details.detail.as_ref())
        {
            code.push_str(detail);
            spans.push(zed::CodeLabelSpan::literal(detail.clone(), None));
        }

        if let Some(description) = completion
            .label_details
            .as_ref()
            .and_then(|details| details.description.as_ref())
            .or(completion.detail.as_ref())
        {
            let suffix = format!(" {description}");
            code.push_str(&suffix);
            spans.push(zed::CodeLabelSpan::literal(suffix, None));
        }

        Some(zed::CodeLabel {
            code,
            spans,
            filter_range: (0..name_length).into(),
        })
    }
}

fn language_server_arguments(
    extension_settings: &Option<zed::serde_json::Value>,
) -> Result<Vec<String>> {
    let mut arguments = vec!["--lsp".into(), "--stdio".into()];

    if let Some(pprof_directory) =
        settings::string_setting(extension_settings, ExtensionSetting::PprofDir)?
    {
        arguments.push("--pprofDir".into());
        arguments.push(pprof_directory);
    }

    if let Some(extra) =
        settings::string_array_setting(extension_settings, ExtensionSetting::ServerArgs)?
    {
        arguments.extend(extra);
    }

    Ok(arguments)
}

/// Builds the server environment with ascending precedence: shell environment,
/// `server.env`, `server.goMemLimit`, then `binary.env`.
fn server_environment(
    worktree: &zed::Worktree,
    extension_settings: &Option<zed::serde_json::Value>,
    binary_env: Option<std::collections::HashMap<String, String>>,
) -> Result<Vec<(String, String)>> {
    let mut env = worktree.shell_env();

    if let Some(extra) =
        settings::string_map_setting(extension_settings, ExtensionSetting::ServerEnv)?
    {
        for (key, value) in extra {
            upsert_environment_variable(&mut env, key, value);
        }
    }

    if let Some(go_memory_limit) =
        settings::string_setting(extension_settings, ExtensionSetting::GoMemLimit)?
    {
        settings::ensure_go_mem_limit(&go_memory_limit)?;
        upsert_environment_variable(&mut env, "GOMEMLIMIT".into(), go_memory_limit);
    }

    if let Some(binary_env) = binary_env {
        for (key, value) in binary_env {
            upsert_environment_variable(&mut env, key, value);
        }
    }

    Ok(env)
}

fn upsert_environment_variable(env: &mut Vec<(String, String)>, key: String, value: String) {
    env.retain(|(existing_key, _)| *existing_key != key);
    env.push((key, value));
}

zed::register_extension!(TsGoExtension);
