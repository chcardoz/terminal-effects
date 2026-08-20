use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const TERMINAL_BROWSER_VERSION: &str = "v0.5.8";

struct Release {
    url: &'static str,
    sha256: &'static str,
    size: u64,
}

pub fn resolve() -> Result<PathBuf> {
    if let Some(override_path) = env::var_os("TE_TERMINAL_BROWSER_BIN") {
        let path = PathBuf::from(override_path);
        if !path.is_file() {
            bail!("TE_TERMINAL_BROWSER_BIN does not exist: {}", path.display());
        }
        return Ok(path);
    }
    if let Some(path) = find_on_path("terminal-browser") {
        return Ok(path);
    }
    let system = data_home()?.join("terminal-browser/app/bin/terminal-browser");
    if system.is_file() {
        return Ok(system);
    }
    let managed = managed_root()?.join("bin/terminal-browser");
    if managed.is_file() && managed_version(&managed)? == TERMINAL_BROWSER_VERSION {
        return Ok(managed);
    }
    install_managed()
}

pub fn managed_status() -> Result<serde_json::Value> {
    let root = managed_root()?;
    let bin = root.join("bin/terminal-browser");
    Ok(serde_json::json!({
        "version": TERMINAL_BROWSER_VERSION,
        "installed": bin.is_file(),
        "path": bin,
        "override": env::var_os("TE_TERMINAL_BROWSER_BIN").map(PathBuf::from),
        "pathInstall": find_on_path("terminal-browser"),
    }))
}

fn managed_version(bin: &Path) -> Result<String> {
    let root = bin
        .parent()
        .and_then(Path::parent)
        .context("terminal-browser binary has no runtime root")?;
    Ok(fs::read_to_string(root.join("VERSION"))
        .unwrap_or_default()
        .trim()
        .to_string())
}

fn install_managed() -> Result<PathBuf> {
    let release = release_for_host()?;
    let root = managed_root()?;
    let parent = root.parent().context("runtime root has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = env::temp_dir().join(format!(
        "terminal-effects-runtime-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&temp)?;
    let cleanup = TempDir(temp.clone());
    let tarball = temp.join("terminal-browser.tar.gz");
    eprintln!(
        "te: installing Chromium terminal runtime {} ({} MB, one time)",
        TERMINAL_BROWSER_VERSION,
        release.size / 1_000_000
    );
    let status = Command::new("curl")
        .args(["-fL", "--retry", "3", "--progress-bar", release.url, "-o"])
        .arg(&tarball)
        .status()
        .context("curl is required to install the Chromium terminal runtime")?;
    if !status.success() {
        bail!("terminal-browser download failed");
    }
    verify_sha256(&tarball, release.sha256)?;
    let staging = parent.join(format!(
        "{}.installing-{}",
        TERMINAL_BROWSER_VERSION,
        std::process::id()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&tarball)
        .args(["-C"])
        .arg(&staging)
        .args(["--strip-components", "1"])
        .status()
        .context("tar is required to unpack the Chromium terminal runtime")?;
    if !status.success() {
        let _ = fs::remove_dir_all(&staging);
        bail!("could not unpack terminal-browser runtime");
    }
    let staged_bin = staging.join("bin/terminal-browser");
    let staged_version = fs::read_to_string(staging.join("VERSION"))
        .context("downloaded runtime is missing VERSION")?;
    if !staged_bin.is_file() || staged_version.trim() != TERMINAL_BROWSER_VERSION {
        let _ = fs::remove_dir_all(&staging);
        bail!("downloaded terminal-browser runtime is incomplete");
    }
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::rename(&staging, &root)?;
    drop(cleanup);
    eprintln!("te: Chromium terminal runtime installed");
    Ok(root.join("bin/terminal-browser"))
}

fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 1024 * 128];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hash.finalize());
    if actual != expected {
        bail!("runtime checksum mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

fn release_for_host() -> Result<Release> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Ok(Release {
            url: "https://terminal-browser.sh/install/dl/stable/v0.5.8/terminal-browser-darwin-arm64.tar.gz",
            sha256: "7bf70a1ba372c4153dd39a8776fb0ecb2d7660a9cd824f6fdbb8e5535800a72c",
            size: 130_138_063,
        }),
        ("linux", "aarch64") => Ok(Release {
            url: "https://terminal-browser.sh/install/dl/stable/v0.5.8/terminal-browser-linux-arm64.tar.gz",
            sha256: "9ffe7fc1f2a309ed0be48c2f35fba534f38163d64c22c0c7dc539949d4f19e71",
            size: 135_193_546,
        }),
        ("linux", "x86_64") => Ok(Release {
            url: "https://terminal-browser.sh/install/dl/stable/v0.5.8/terminal-browser-linux-x64.tar.gz",
            sha256: "c330be3341ef6f6cb106e4fb32c1d60754a08e1a7641143a7a6a4d9e9448f617",
            size: 137_351_100,
        }),
        (os, arch) => {
            bail!("terminal-browser {TERMINAL_BROWSER_VERSION} does not support {os}-{arch}")
        }
    }
}

fn managed_root() -> Result<PathBuf> {
    Ok(data_home()?
        .join("terminal-effects/runtime/terminal-browser")
        .join(TERMINAL_BROWSER_VERSION))
}

fn data_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_DATA_HOME").map(PathBuf::from)
        && path.is_absolute()
    {
        return Ok(path);
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
    Ok(home.join(".local/share"))
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_exists_for_test_host_when_supported() {
        if matches!(env::consts::OS, "macos" | "linux") {
            assert!(!release_for_host().unwrap().sha256.is_empty());
        }
    }
}
