import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const APPARMOR_SCRIPT = path.resolve(__dirname, "..", "..", "scripts", "apparmor.sh");

function kernelSetting(file: string): string | null {
  if (!fs.existsSync(file)) return null;
  return fs.readFileSync(file, "utf8").trim();
}

function setuidSandbox(electron: string): boolean {
  const helper = path.join(path.dirname(electron), "chrome-sandbox");
  const stat = fs.statSync(helper, { throwIfNoEntry: false });
  return stat !== undefined && stat.uid === 0 && (stat.mode & 0o4000) !== 0;
}

function apparmorProfile(electron: string): boolean {
  try {
    const resolved = fs.realpathSync(electron);
    const slug = crypto.createHash("sha256").update(resolved).digest("hex").slice(0, 12);
    return fs
      .readFileSync(`/etc/apparmor.d/terminal-effects-renderer-${slug}`, "utf8")
      .includes(resolved);
  } catch {
    return false;
  }
}

export function linuxSandboxError(electron: string): string | null {
  if (process.getuid?.() === 0) {
    return "Chromium's Linux sandbox cannot run as root; run Terminal Effects as a non-root user";
  }
  if (setuidSandbox(electron) || apparmorProfile(electron)) return null;
  if (kernelSetting("/proc/sys/kernel/apparmor_restrict_unprivileged_userns") === "1") {
    return "AppArmor blocks the user namespaces required by the Chromium renderer";
  }
  if (kernelSetting("/proc/sys/kernel/unprivileged_userns_clone") === "0") {
    return "Linux disables the user namespaces required by the Chromium renderer";
  }
  return null;
}

export function installApparmorProfile(electron: string): void {
  execFileSync("bash", [APPARMOR_SCRIPT, electron], { stdio: "inherit" });
}
