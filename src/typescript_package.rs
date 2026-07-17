use zed_extension_api::{self as zed, Result};

pub const TYPESCRIPT_PACKAGE: &str = "typescript";

/// Finds a usable project-local TypeScript 7+ package by scanning the root
/// package manifest. npm aliases may use any dependency key, but their target
/// package must be `typescript`.
pub fn find_local_typescript_package_dir(worktree: &zed::Worktree) -> Option<String> {
    let content = worktree.read_text_file("package.json").ok()?;
    let pkg: zed::serde_json::Value = zed::serde_json::from_str(&content).ok()?;

    let root = worktree
        .root_path()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();

    find_typescript_dependency(&pkg, &root, |key, _spec, _directory| {
        let package_json_path = format!("node_modules/{key}/package.json");
        match worktree.read_text_file(&package_json_path) {
            Ok(package_json) => {
                typescript_version_from_package_json(&package_json, &package_json_path)
                    .and_then(|version| ensure_typescript_7_or_newer(&version))
                    .is_ok()
            }
            // Excluded directories such as node_modules are not always exposed
            // through Worktree::read_text_file. The manifest declaration was
            // already checked above, so let Microsoft's launcher resolve it.
            Err(_) => true,
        }
    })
}

fn find_typescript_dependency(
    pkg: &zed::serde_json::Value,
    root: &str,
    mut package_is_usable: impl FnMut(&str, &str, &str) -> bool,
) -> Option<String> {
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        let Some(dependencies) = pkg.get(section).and_then(|value| value.as_object()) else {
            continue;
        };

        for (key, value) in dependencies {
            let Some(spec) = value.as_str() else {
                continue;
            };
            if dependency_package_name(key, spec) != Some(TYPESCRIPT_PACKAGE) {
                continue;
            }
            if !declared_spec_may_resolve_to_typescript_7(spec) {
                continue;
            }

            let dir = format!("{root}/node_modules/{key}");
            if package_is_usable(key, spec, &dir) {
                return Some(dir);
            }
        }
    }

    None
}

fn dependency_package_name<'a>(key: &'a str, spec: &'a str) -> Option<&'a str> {
    let spec = spec.trim();
    let Some(alias) = spec.strip_prefix("npm:") else {
        return Some(key);
    };

    let version_separator = if let Some(scoped_alias) = alias.strip_prefix('@') {
        scoped_alias.rfind('@').map(|position| position + 1)
    } else {
        alias.rfind('@')
    };
    let package_name = version_separator.map_or(alias, |position| &alias[..position]);
    (!package_name.is_empty()).then_some(package_name)
}

/// Rejects declarations that clearly select TypeScript 6 while allowing tags,
/// broader ranges, and non-registry specs whose installed version cannot be
/// known from `package.json` alone.
fn declared_spec_may_resolve_to_typescript_7(spec: &str) -> bool {
    let spec = dependency_version_spec(spec).trim();
    if spec.is_empty() {
        return true;
    }

    if let Some((_, upper_bound)) = spec.split_once('<') {
        let upper_bound = upper_bound.trim();
        let is_inclusive = upper_bound.starts_with('=');
        let upper_bound = upper_bound.trim_start_matches('=').trim();
        if leading_major(upper_bound)
            .is_some_and(|major| major < 7 || (major == 7 && !is_inclusive))
        {
            return false;
        }
    }

    let bounded_from_major = spec
        .trim_start_matches(['v', '^', '~', '=', ' '])
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
        && !spec.starts_with('>')
        && !spec.contains("||");
    if bounded_from_major {
        return leading_major(spec).is_none_or(|major| major >= 7);
    }

    true
}

fn dependency_version_spec(spec: &str) -> &str {
    let spec = spec.trim();
    let Some(alias) = spec.strip_prefix("npm:") else {
        return spec;
    };

    let separator = if let Some(scoped_alias) = alias.strip_prefix('@') {
        scoped_alias.rfind('@').map(|position| position + 1)
    } else {
        alias.rfind('@')
    };
    separator.map_or("", |position| &alias[position + 1..])
}

fn leading_major(spec: &str) -> Option<u64> {
    let digits: String = spec
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(|character| character.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn typescript_version_from_package_json(content: &str, path: &str) -> Result<String> {
    let pkg: zed::serde_json::Value =
        zed::serde_json::from_str(content).map_err(|error| format!("invalid {path}: {error}"))?;
    pkg.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no version in {path}"))
}

/// Locates the native `tsc` executable in the platform package next to (or
/// inside) the resolved TypeScript package. Callers fall back to Microsoft's
/// Node launcher when an install layout cannot be resolved directly.
pub fn find_native_server_binary(worktree: &zed::Worktree, package_dir: &str) -> Option<String> {
    if let Some(relative_directory) = worktree_relative_directory(worktree, package_dir) {
        let relative_binary = find_worktree_native_server_binary(worktree, &relative_directory)?;
        return Some(absolute_worktree_path(worktree, &relative_binary));
    }

    let (platform_package, executable) = platform_package_and_executable()?;
    let candidates = [
        format!("{package_dir}/lib/{executable}"),
        format!("{package_dir}/../{platform_package}/lib/{executable}"),
        format!("{package_dir}/node_modules/{platform_package}/lib/{executable}"),
    ];

    candidates
        .into_iter()
        .find(|path| std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()))
}

fn find_worktree_native_server_binary(
    worktree: &zed::Worktree,
    relative_package_directory: &str,
) -> Option<String> {
    let (platform_package, executable) = platform_package_and_executable()?;
    let candidates = [
        relative_package_directory.to_string(),
        format!("node_modules/{platform_package}"),
        format!("{relative_package_directory}/node_modules/{platform_package}"),
    ];

    candidates.into_iter().find_map(|candidate| {
        let package_json_path = format!("{candidate}/package.json");
        let package_json = worktree.read_text_file(&package_json_path).ok()?;
        let package: zed::serde_json::Value = zed::serde_json::from_str(&package_json).ok()?;
        let package_name = package.get("name").and_then(|name| name.as_str());

        let is_platform_package =
            candidate != relative_package_directory || package_name == Some(&platform_package);
        is_platform_package.then(|| format!("{candidate}/lib/{executable}"))
    })
}

fn platform_package_and_executable() -> Option<(String, &'static str)> {
    let (os, arch) = zed::current_platform();
    let platform = match os {
        zed::Os::Mac => "darwin",
        zed::Os::Linux => "linux",
        zed::Os::Windows => "win32",
    };
    let arch = match arch {
        zed::Architecture::Aarch64 => "arm64",
        zed::Architecture::X8664 => "x64",
        zed::Architecture::X86 => return None,
    };
    let exe = match os {
        zed::Os::Windows => "tsc.exe",
        _ => "tsc",
    };

    Some((format!("@typescript/typescript-{platform}-{arch}"), exe))
}

pub fn node_shim_path(worktree: &zed::Worktree, package_dir: &str) -> Result<String> {
    let shim = format!("{package_dir}/bin/tsc");
    let is_in_worktree = worktree_relative_directory(worktree, package_dir).is_some();
    let exists =
        is_in_worktree || std::fs::metadata(&shim).is_ok_and(|metadata| metadata.is_file());

    if exists || !path_is_in_extension_work_directory(package_dir) {
        Ok(shim)
    } else {
        Err(format!(
            "TypeScript package at `{package_dir}` has no native server binary for this platform and no `bin/tsc` launcher"
        ))
    }
}

fn normalized_worktree_root(worktree: &zed::Worktree) -> String {
    let root = worktree.root_path().replace('\\', "/");
    if root == "/" {
        root
    } else {
        root.trim_end_matches('/').to_string()
    }
}

fn absolute_worktree_path(worktree: &zed::Worktree, relative_path: &str) -> String {
    let root = normalized_worktree_root(worktree);
    if root == "/" {
        format!("/{relative_path}")
    } else {
        format!("{root}/{relative_path}")
    }
}

fn worktree_relative_directory(worktree: &zed::Worktree, directory: &str) -> Option<String> {
    let root = normalized_worktree_root(worktree);
    let directory = directory.replace('\\', "/");
    if directory == root {
        return Some(String::new());
    }

    let prefix = if root == "/" {
        root
    } else {
        format!("{root}/")
    };
    if matches!(zed::current_platform().0, zed::Os::Windows) {
        directory
            .to_ascii_lowercase()
            .strip_prefix(&prefix.to_ascii_lowercase())
            .map(|relative| {
                let start = directory.len() - relative.len();
                directory[start..].to_string()
            })
    } else {
        directory.strip_prefix(&prefix).map(|path| path.to_string())
    }
}

fn path_is_in_extension_work_directory(path: &str) -> bool {
    let Ok(current_directory) = std::env::current_dir() else {
        return false;
    };
    let current_directory = current_directory
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    let path = path.replace('\\', "/");
    path == current_directory || path.starts_with(&format!("{current_directory}/"))
}

pub fn ensure_typescript_7_or_newer(version: &str) -> Result<()> {
    let version_without_prefix = version.strip_prefix('v').unwrap_or(version);
    let major_part = version_without_prefix
        .split(|character: char| !character.is_ascii_digit())
        .next()
        .unwrap_or("");
    if major_part.is_empty() {
        return Err(format!("invalid TypeScript version `{version}`"));
    }
    let major: u64 = major_part
        .parse()
        .map_err(|_| format!("invalid TypeScript version `{version}`"))?;
    if major < 7 {
        return Err(format!(
            "TypeScript LSP requires TypeScript 7 or newer, got `{version}`"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new(name: &str) -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);

            let root = std::env::temp_dir().join(format!(
                "tsgo-{name}-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).expect("create test project directory");
            Self { root }
        }

        fn add_package(&self, key: &str, version: &str, has_launcher: bool) -> String {
            let package_dir = self.root.join("node_modules").join(key);
            std::fs::create_dir_all(&package_dir).expect("create test package directory");
            std::fs::write(
                package_dir.join("package.json"),
                format!(r#"{{"version":"{version}"}}"#),
            )
            .expect("write test package.json");

            if has_launcher {
                let bin_dir = package_dir.join("bin");
                std::fs::create_dir_all(&bin_dir).expect("create test package bin directory");
                std::fs::write(bin_dir.join("tsc"), "").expect("write test tsc launcher");
            }

            package_dir
                .to_string_lossy()
                .into_owned()
                .replace('\\', "/")
        }

        fn root(&self) -> String {
            self.root.to_string_lossy().into_owned().replace('\\', "/")
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn test_package_is_usable(_key: &str, _spec: &str, directory: &str) -> bool {
        let package_json_path = format!("{directory}/package.json");
        let Ok(package_json) = std::fs::read_to_string(&package_json_path) else {
            return false;
        };
        let Ok(version) = typescript_version_from_package_json(&package_json, &package_json_path)
        else {
            return false;
        };

        ensure_typescript_7_or_newer(&version).is_ok()
            && std::fs::metadata(format!("{directory}/bin/tsc"))
                .is_ok_and(|metadata| metadata.is_file())
    }

    #[test]
    fn parses_dependency_package_names() {
        let cases = [
            (("typescript", "^7.0.2"), Some("typescript")),
            (("typescript", "^8.0.0"), Some("typescript")),
            (
                ("@typescript/native", "npm:typescript@^7.0.2"),
                Some("typescript"),
            ),
            (("whatever", " npm:typescript@next "), Some("typescript")),
            (("foo", "^7.2.0"), Some("foo")),
            (("foo", "npm:bar@7.0.0"), Some("bar")),
            (
                ("typescript", "npm:@typescript/typescript6@^6.0.2"),
                Some("@typescript/typescript6"),
            ),
            (("foo", "npm:@scope/package@next"), Some("@scope/package")),
            (("foo", "npm:@scope/package"), Some("@scope/package")),
            (("foo", "npm:"), None),
        ];

        for ((key, spec), expected) in cases {
            assert_eq!(dependency_package_name(key, spec), expected);
        }
    }

    #[test]
    fn identifies_declarations_that_can_select_typescript_7() {
        for spec in [
            "7.0.2",
            "^7",
            "~7.1",
            "next",
            "latest",
            ">=6",
            ">=6 <8",
            "<=7",
            "^6 || ^7",
            "workspace:^6",
            "npm:typescript@^7",
        ] {
            assert!(
                declared_spec_may_resolve_to_typescript_7(spec),
                "{spec} should be accepted"
            );
        }

        for spec in [
            "6.0.2",
            "^6",
            "~6.1",
            "6.x",
            "<7",
            "<=6",
            ">=6 <7",
            "npm:typescript@^6",
        ] {
            assert!(
                !declared_spec_may_resolve_to_typescript_7(spec),
                "{spec} should be rejected"
            );
        }
    }

    #[test]
    fn local_resolution_ignores_unrelated_packages_and_accepts_typescript_8() {
        let project = TestProject::new("package-identity");
        project.add_package("foo", "7.2.0", true);
        let typescript_dir = project.add_package("typescript", "8.0.0", true);
        let manifest = zed::serde_json::json!({
            "dependencies": {
                "foo": "^7.2.0",
                "typescript": "^8.0.0"
            }
        });

        assert_eq!(
            find_typescript_dependency(&manifest, &project.root(), test_package_is_usable),
            Some(typescript_dir)
        );
    }

    #[test]
    fn local_resolution_supports_side_by_side_aliases() {
        let project = TestProject::new("side-by-side-aliases");
        let native_dir = project.add_package("@typescript/native", "7.0.2", true);
        project.add_package("typescript", "6.0.2", true);
        let manifest = zed::serde_json::json!({
            "devDependencies": {
                "@typescript/native": "npm:typescript@^7.0.2",
                "typescript": "npm:@typescript/typescript6@^6.0.2"
            }
        });

        assert_eq!(
            find_typescript_dependency(&manifest, &project.root(), test_package_is_usable),
            Some(native_dir)
        );
    }

    #[test]
    fn local_resolution_continues_after_an_outdated_alias() {
        let project = TestProject::new("continue-after-outdated");
        project.add_package("a-typescript", "6.0.2", true);
        let usable_dir = project.add_package("z-typescript", "7.0.2", true);
        let manifest = zed::serde_json::json!({
            "dependencies": {
                "a-typescript": "npm:typescript@6.0.2",
                "z-typescript": "npm:typescript@7.0.2"
            }
        });

        assert_eq!(
            find_typescript_dependency(&manifest, &project.root(), test_package_is_usable),
            Some(usable_dir)
        );
    }

    #[test]
    fn validates_supported_typescript_versions() {
        assert!(ensure_typescript_7_or_newer("7.0.0").is_ok());
        assert!(ensure_typescript_7_or_newer("7.1.0-beta.1").is_ok());
        assert!(ensure_typescript_7_or_newer("10.0.0").is_ok());
        assert!(ensure_typescript_7_or_newer("v7.0").is_ok());
        assert!(ensure_typescript_7_or_newer("6.9.9").is_err());
        assert!(ensure_typescript_7_or_newer("foo").is_err());
    }
}
