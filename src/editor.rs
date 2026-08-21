use crate::{runtime, server::EditorServer};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{IsTerminal, stdout};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PRELOAD: &str = r#""use strict";
const { contextBridge } = require("electron");
const host = globalThis.terminalEffectsRenderer;
contextBridge.exposeInMainWorld("terminalEffectsHost", {
  quit: () => host && typeof host.quit === "function" && host.quit(),
  theme: () => host && typeof host.theme === "function" ? host.theme() : null,
});
"#;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub pid: u32,
    pub project: String,
    pub url: String,
    pub renderer: String,
}

struct SessionGuard(PathBuf);

impl SessionGuard {
    fn write(root: &Path, project_path: &Path, url: &str) -> Result<Self> {
        let path = root.join(".te/session.json");
        fs::create_dir_all(root.join(".te"))?;
        fs::write(
            &path,
            serde_json::to_vec_pretty(&SessionInfo {
                pid: std::process::id(),
                project: project_path.display().to_string(),
                url: url.to_string(),
                renderer: "chromium-offscreen".into(),
            })?,
        )?;
        Ok(Self(path))
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn run(project_path: &Path) -> Result<()> {
    if !std::io::stdin().is_terminal() || !stdout().is_terminal() {
        bail!(
            "`te .` needs an interactive terminal; use `te serve .` for a regular browser or agent commands without a UI"
        );
    }
    let root = project_path
        .parent()
        .context("project file has no parent")?;
    let renderer = runtime::resolve()?;
    let server = EditorServer::start(project_path, 0)?;
    let preload = write_preload(root)?;
    let _session = SessionGuard::write(root, project_path, &server.url)?;
    let status = Command::new(renderer)
        .arg(&server.url)
        .arg("--app-mode")
        .arg(format!("--preload={}", preload.display()))
        .current_dir(root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("could not start the Terminal Effects renderer")?;
    drop(server);
    if !status.success() {
        bail!("Chromium terminal renderer exited with {status}");
    }
    Ok(())
}

pub fn serve(project_path: &Path, port: u16) -> Result<()> {
    let server = EditorServer::start(project_path, port)?;
    println!("{}", server.url);
    server.wait()
}

pub fn session_running(root: &Path) -> bool {
    let Ok(bytes) = fs::read(root.join(".te/session.json")) else {
        return false;
    };
    let Ok(session) = serde_json::from_slice::<SessionInfo>(&bytes) else {
        return false;
    };
    Command::new("kill")
        .args(["-0", &session.pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn write_preload(root: &Path) -> Result<PathBuf> {
    let path = root.join(".te/runtime/terminal-effects-preload.cjs");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, PRELOAD)?;
    Ok(path)
}
