use zed_extension_api::{self as zed, Result, serde_json};

#[derive(Debug, serde::Deserialize)]
struct Process {
    platform: String,
    arch: String,
}

fn get_node_process() -> Result<Process> {
    let output = zed::Command {
        command: "node".into(),
        args: vec!["-p".into(), "JSON.stringify(process)".into()],
        env: vec![],
    }
    .output()?;

    if !output.status.is_some_and(|status| status == 0) {
        return Err(format!("unable to get the node process variable"));
    }

    serde_json::from_slice::<Process>(output.stdout.as_slice()).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerType {
    Node,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub package_name: String,
    pub server_path: String,
    pub server_type: ServerType,
}

fn get_package_name(process: &Process) -> String {
    format!(
        "@typescript/native-preview-{}-{}",
        process.platform, process.arch,
    )
}

fn get_server_path(process: &Process) -> String {
    format!(
        "node_modules/{}/lib/tsgo{}",
        get_package_name(process),
        if process.platform == "win32" {
            ".exe"
        } else {
            ""
        }
    )
}

pub fn get_native_package() -> Option<Package> {
    get_node_process().ok().map(|process| Package {
        package_name: get_package_name(&process),
        server_path: get_server_path(&process),
        server_type: ServerType::Native,
    })
}

pub fn get_node_package() -> Package {
    Package {
        package_name: "@typescript/native-preview".into(),
        server_path: "node_modules/@typescript/native-preview/bin/tsgo.js".into(),
        server_type: ServerType::Node,
    }
}
