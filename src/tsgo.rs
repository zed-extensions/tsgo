use std::fs;

use package_utils::{Package, ServerType, get_native_package, get_node_package};
use zed_extension_api::{self as zed, Result, settings::LspSettings};

mod package_utils;

struct TsGoExtension {
    cached_package: Option<Package>,
}

impl TsGoExtension {
    fn server_exists(&self, package: &Package) -> bool {
        fs::metadata(package.server_path.clone()).map_or(false, |stat| stat.is_file())
    }

    fn binary_path(&mut self, package: &Package, id: &zed::LanguageServerId) -> Result<Package> {
        let server_exists = self.server_exists(package);
        if self
            .cached_package
            .as_ref()
            .is_some_and(|cached_package| cached_package == package)
            && server_exists
        {
            return Ok(package.clone());
        }

        zed::set_language_server_installation_status(
            id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let version = zed::npm_package_latest_version(&package.package_name)?;

        if !server_exists
            || zed::npm_package_installed_version(&package.package_name)?.as_ref() != Some(&version)
        {
            zed::set_language_server_installation_status(
                id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );
            let result = zed::npm_install_package(&package.package_name, &version);
            match result {
                Ok(()) => {
                    if !self.server_exists(package) {
                        Err(format!(
                            "installed package '{}' did not contain expected path '{}'",
                            &package.package_name, &package.server_path
                        ))?;
                    }
                }
                Err(error) => {
                    if !self.server_exists(package) {
                        Err(error)?;
                    }
                }
            }
        }
        Ok(package.clone())
    }
}

impl zed::Extension for TsGoExtension {
    fn new() -> Self {
        Self {
            cached_package: None,
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
        let pkg = get_native_package()
            .and_then(|package| self.binary_path(&package, language_server_id).ok())
            .map_or_else(
                || self.binary_path(&get_node_package(), language_server_id),
                |x| Ok(x),
            )?;
        self.cached_package = Some(pkg.clone());
        let filepath = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(pkg.server_path)
            .to_string_lossy()
            .into_owned();
        Ok(match pkg.server_type {
            ServerType::Native => zed::Command {
                command: filepath,
                args: vec!["--lsp".into(), "--stdio".into()],
                env: env.into_iter().flat_map(|env| env.into_iter()).collect(),
            },
            ServerType::Node => zed::Command {
                command: zed::node_binary_path()?,
                args: vec![filepath, "--lsp".into(), "--stdio".into()],
                env: env.into_iter().flat_map(|env| env.into_iter()).collect(),
            },
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
