use std::cell::OnceCell;

use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree, settings::LspSettings};

pub const FALLBACK_LANGUAGE_SERVER_ID: &str = "tsgo";

#[derive(Clone, Copy)]
pub enum ExtensionSetting {
    PackageVersion,
    Version,
    UpdateChannel,
    TsdkPath,
    PprofDir,
    GoMemLimit,
    ServerArgs,
    ServerEnv,
}

impl ExtensionSetting {
    fn path(self) -> &'static str {
        match self {
            Self::PackageVersion => "package_version",
            Self::Version => "version",
            Self::UpdateChannel => "updateChannel",
            Self::TsdkPath => "tsdk.path",
            Self::PprofDir => "server.pprofDir",
            Self::GoMemLimit => "server.goMemLimit",
            Self::ServerArgs => "server.args",
            Self::ServerEnv => "server.env",
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

pub fn string_array_setting(
    settings: &Option<zed::serde_json::Value>,
    setting: ExtensionSetting,
) -> Result<Option<Vec<String>>> {
    let Some(value) = setting_value(settings, setting) else {
        return Ok(None);
    };
    let error = || {
        format!(
            "setting `{}` must be an array of strings, got: {value}",
            setting.path()
        )
    };
    let items = value.as_array().ok_or_else(error)?;
    items
        .iter()
        .map(|item| item.as_str().map(|s| s.to_string()).ok_or_else(error))
        .collect::<Result<Vec<String>>>()
        .map(Some)
}

pub fn string_map_setting(
    settings: &Option<zed::serde_json::Value>,
    setting: ExtensionSetting,
) -> Result<Option<Vec<(String, String)>>> {
    let Some(value) = setting_value(settings, setting) else {
        return Ok(None);
    };
    let object = value.as_object().ok_or_else(|| {
        format!(
            "setting `{}` must be an object of string values, got: {value}",
            setting.path()
        )
    })?;
    object
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|s| (key.clone(), s.to_string()))
                .ok_or_else(|| {
                    format!(
                        "setting `{}.{key}` must be a string, got: {value}",
                        setting.path()
                    )
                })
        })
        .collect::<Result<Vec<(String, String)>>>()
        .map(Some)
}

/// Validates `GOMEMLIMIT` according to the Go runtime's accepted syntax.
pub fn ensure_go_mem_limit(value: &str) -> Result<()> {
    if value == "off" {
        return Ok(());
    }

    let digits_end = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, suffix) = value.split_at(digits_end);

    if number.is_empty() {
        return Err(format!(
            "invalid GOMEMLIMIT value `{value}`: expected an integer byte count with an optional B/KiB/MiB/GiB/TiB suffix, or `off`"
        ));
    }

    match suffix {
        "" | "B" | "KiB" | "MiB" | "GiB" | "TiB" => Ok(()),
        _ => Err(format!(
            "invalid GOMEMLIMIT value `{value}`: the Go runtime only accepts B/KiB/MiB/GiB/TiB suffixes"
        )),
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

    #[test]
    fn string_array_setting_accepts_strings_only() {
        let settings = Some(json!({"server": {"args": ["--pprofDir", "./p"]}}));
        assert_eq!(
            string_array_setting(&settings, ExtensionSetting::ServerArgs).unwrap(),
            Some(vec!["--pprofDir".to_string(), "./p".to_string()])
        );

        let settings = Some(json!({"server": {"args": ["ok", 5]}}));
        assert!(string_array_setting(&settings, ExtensionSetting::ServerArgs).is_err());
    }

    #[test]
    fn string_map_setting_accepts_strings_only() {
        let settings = Some(json!({"server": {"env": {"GOGC": "50"}}}));
        assert_eq!(
            string_map_setting(&settings, ExtensionSetting::ServerEnv).unwrap(),
            Some(vec![("GOGC".to_string(), "50".to_string())])
        );

        let settings = Some(json!({"server": {"env": {"GOGC": 50}}}));
        assert!(string_map_setting(&settings, ExtensionSetting::ServerEnv).is_err());
    }

    #[test]
    fn validates_go_mem_limit() {
        assert!(ensure_go_mem_limit("2048MiB").is_ok());
        assert!(ensure_go_mem_limit("1024").is_ok());
        assert!(ensure_go_mem_limit("1GiB").is_ok());
        assert!(ensure_go_mem_limit("off").is_ok());
        assert!(ensure_go_mem_limit("1.5GiB").is_err());
        assert!(ensure_go_mem_limit("2048MB").is_err());
        assert!(ensure_go_mem_limit("foo").is_err());
        assert!(ensure_go_mem_limit(".5GiB").is_err());
        assert!(ensure_go_mem_limit("GiB").is_err());
    }
}
