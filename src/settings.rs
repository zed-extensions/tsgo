use std::cell::OnceCell;

use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree, settings::LspSettings};

pub const FALLBACK_LANGUAGE_SERVER_ID: &str = "tsgo";

#[derive(Clone, Copy)]
pub enum ExtensionSetting {
    PackageVersion,
    Version,
    UpdateChannel,
    TsdkPath,
}

impl ExtensionSetting {
    fn path(self) -> &'static str {
        match self {
            Self::PackageVersion => "package_version",
            Self::Version => "version",
            Self::UpdateChannel => "updateChannel",
            Self::TsdkPath => "tsdk.path",
        }
    }
}

pub struct LspSettingsWithFallback<'a> {
    settings: LspSettings,
    fallback_settings: OnceCell<Option<LspSettings>>,
    worktree: &'a Worktree,
}

impl<'a> LspSettingsWithFallback<'a> {
    pub fn for_worktree(server_id: &LanguageServerId, worktree: &'a Worktree) -> zed::Result<Self> {
        LspSettings::for_worktree(server_id.as_ref(), worktree).map(|settings| Self {
            settings,
            fallback_settings: OnceCell::new(),
            worktree,
        })
    }

    pub fn get_setting<F, R>(&self, f: F) -> Option<&R>
    where
        F: Fn(&LspSettings) -> Option<&R>,
    {
        f(&self.settings).or_else(|| {
            self.fallback_settings
                .get_or_init(|| {
                    LspSettings::for_worktree(FALLBACK_LANGUAGE_SERVER_ID, self.worktree).ok()
                })
                .as_ref()
                .and_then(f)
        })
    }

    pub fn into_setting<F, R>(self, f: F) -> Option<R>
    where
        F: Fn(LspSettings) -> Option<R>,
    {
        f(self.settings).or_else(|| {
            let _ = self.fallback_settings.get_or_init(|| {
                LspSettings::for_worktree(FALLBACK_LANGUAGE_SERVER_ID, self.worktree).ok()
            });
            self.fallback_settings.into_inner().flatten().and_then(f)
        })
    }
}

/// Looks a setting up by its dotted path, accepting both nested and literal
/// dotted-key forms. The nested form wins when both are set.
fn setting_value(
    settings: &Option<zed::serde_json::Value>,
    setting: ExtensionSetting,
) -> Option<&zed::serde_json::Value> {
    let object = settings.as_ref()?.as_object()?;
    let dotted_path = setting.path();

    let mut parts = dotted_path.split('.');
    let first = parts.next()?;
    let nested = object.get(first).and_then(|mut value| {
        for part in parts {
            value = value.as_object()?.get(part)?;
        }
        Some(value)
    });

    nested.or_else(|| object.get(dotted_path))
}

pub fn string_setting(
    settings: &Option<zed::serde_json::Value>,
    setting: ExtensionSetting,
) -> Result<Option<String>> {
    match setting_value(settings, setting) {
        None => Ok(None),
        Some(value) => value.as_str().map(|s| Some(s.to_string())).ok_or_else(|| {
            format!(
                "setting `{}` must be a string, got: {value}",
                setting.path()
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zed_extension_api::serde_json::json;

    #[test]
    fn string_setting_nested_wins_and_type_errors() {
        let settings = Some(json!({
            "tsdk": {"path": "./nested"},
            "tsdk.path": "./dotted",
        }));
        assert_eq!(
            string_setting(&settings, ExtensionSetting::TsdkPath).unwrap(),
            Some("./nested".to_string())
        );

        let settings = Some(json!({"tsdk": {"path": 5}}));
        assert!(string_setting(&settings, ExtensionSetting::TsdkPath).is_err());
    }
}
