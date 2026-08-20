use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const RENDERER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
struct Renderer {
    path: PathBuf,
    source: &'static str,
}

pub fn resolve() -> Result<PathBuf> {
    let renderer = locate().with_context(|| {
            "the packaged Terminal Effects renderer is missing. Install a complete Terminal Effects release, run `pnpm install && pnpm build:runtime` in renderer/ for development, or use `te serve .`"
        })?;
    if renderer.source != "override"
        && runtime_version(&renderer.path).as_deref() != Some(RENDERER_VERSION)
    {
        bail!(
            "the packaged renderer does not match te {}. Reinstall the complete Terminal Effects package",
            RENDERER_VERSION
        );
    }
    Ok(renderer.path)
}

pub fn status() -> serde_json::Value {
    let renderer = locate();
    let version = renderer
        .as_ref()
        .and_then(|renderer| runtime_version(&renderer.path));
    serde_json::json!({
        "available": renderer.is_some(),
        "path": renderer.as_ref().map(|renderer| &renderer.path),
        "source": renderer.as_ref().map(|renderer| renderer.source),
        "version": version,
        "expectedVersion": RENDERER_VERSION,
        "override": env::var_os("TE_RENDERER_BIN").map(PathBuf::from),
    })
}

fn locate() -> Option<Renderer> {
    if let Some(path) = env::var_os("TE_RENDERER_BIN").map(PathBuf::from) {
        return executable(&path).then_some(Renderer {
            path,
            source: "override",
        });
    }

    if let Ok(current_binary) = env::current_exe()
        && let Some(directory) = current_binary.parent()
    {
        for relative in [
            "../libexec/terminal-effects-renderer/bin/te-renderer",
            "../libexec/terminal-effects/renderer/bin/te-renderer",
            "../runtime/terminal-effects-renderer/bin/te-renderer",
        ] {
            let path = directory.join(relative);
            if executable(&path) {
                return Some(Renderer {
                    path,
                    source: "package",
                });
            }
        }
    }

    if let Some(path) = find_on_path("te-renderer") {
        return Some(Renderer {
            path,
            source: "path",
        });
    }

    let development = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("renderer/dist/terminal-effects-renderer/bin/te-renderer");
    executable(&development).then_some(Renderer {
        path: development,
        source: "development",
    })
}

fn runtime_version(binary: &Path) -> Option<String> {
    let root = binary.parent()?.parent()?;
    fs::read_to_string(root.join("VERSION"))
        .ok()
        .map(|version| version.trim().to_string())
        .filter(|version| !version.is_empty())
}

fn executable(path: &Path) -> bool {
    path.is_file()
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|candidate| executable(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_version_next_to_renderer() {
        let temporary = tempfile::tempdir().unwrap();
        let binary = temporary.path().join("bin/te-renderer");
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(&binary, "").unwrap();
        fs::write(temporary.path().join("VERSION"), "v1\n").unwrap();
        assert_eq!(runtime_version(&binary).as_deref(), Some("v1"));
    }
}
