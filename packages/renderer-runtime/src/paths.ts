import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const HOME = os.homedir();


function base(variable: string, fallback: string): string {
  const value = process.env[variable];
  return value && path.isAbsolute(value) ? value : path.join(HOME, fallback);
}

const DATA_HOME = base("XDG_DATA_HOME", ".local/share");
const STATE_HOME = base("XDG_STATE_HOME", ".local/state");

const RUNTIME_HOME = process.env.XDG_RUNTIME_DIR ?? STATE_HOME;

function installRoot(): { root: string; dev: boolean } {
  const dist = process.env.TE_RENDERER_DIST_ROOT;
  if (dist) return { root: physical(dist), dev: false };
  for (let dir = __dirname; path.dirname(dir) !== dir; dir = path.dirname(dir)) {
    if (fs.existsSync(path.join(dir, "pnpm-workspace.yaml"))) return { root: dir, dev: true };
  }
  return { root: physical(__dirname), dev: true };
}

function physical(dir: string): string {
  try {
    return fs.realpathSync(dir);
  } catch {
    return dir;
  }
}

export const INSTALL_ROOT = installRoot();

const suffix = crypto.createHash("sha256").update(INSTALL_ROOT.root).digest("hex").slice(0, 8);

export const APP_DIR_NAME = `terminal-effects-renderer${INSTALL_ROOT.dev ? "-dev" : ""}-${suffix}`;

export const DATA_DIR = path.join(DATA_HOME, APP_DIR_NAME);
export const LOGS_DIR = path.join(STATE_HOME, APP_DIR_NAME, "logs");
export const DAEMON_SOCKET = path.join(RUNTIME_HOME, APP_DIR_NAME, "daemon.sock");

export function ensureDataDir(): void {
  fs.mkdirSync(DATA_DIR, { recursive: true });
  fs.writeFileSync(path.join(DATA_DIR, "install"), `${INSTALL_ROOT.root}\n`);
}
