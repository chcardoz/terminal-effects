#!/usr/bin/env node
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";

import { DAEMON_SOCKET, LOGS_DIR, ensureDataDir } from "@terminal-effects/renderer-runtime";
import {
  checkTerminal,
  detect,
  unsupportedGraphicsMessage,
} from "@terminal-effects/terminal-adapters";
import { installApparmorProfile, linuxSandboxError } from "./sandbox";

const DIST_ROOT = process.env.TE_RENDERER_DIST_ROOT ?? null;
delete process.env.ELECTRON_RUN_AS_NODE;

function fail(message: string): never {
  process.stderr.write(`te-renderer: ${message}\n`);
  process.exit(1);
}

const sleep = (milliseconds: number) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const ELECTRON_DIST_BIN =
  process.platform === "darwin"
    ? ["Terminal Effects Renderer.app", "Contents", "MacOS", "Terminal Effects Renderer"]
    : ["electron"];

function rendererDirectory(): string {
  return path.resolve(__dirname, "..", "..");
}

function electronBinary(): string {
  const root = DIST_ROOT ?? rendererDirectory();
  return path.join(root, "electron", ...ELECTRON_DIST_BIN);
}

function browserMain(): string {
  const root = DIST_ROOT ?? rendererDirectory();
  return path.join(root, "browser", "dist", "main.js");
}

function browserLaunchCommand(): { command: string[]; cwd: string } {
  const root = DIST_ROOT ?? rendererDirectory();
  const electron = electronBinary();
  const main = browserMain();
  for (const required of [electron, main]) {
    if (!fs.existsSync(required)) fail(`packaged renderer is missing ${required}`);
  }
  if (process.platform === "linux") {
    let sandboxError = linuxSandboxError(electron);
    if (sandboxError) {
      try {
        installApparmorProfile(electron);
      } catch {}
      sandboxError = linuxSandboxError(electron);
    }
    if (sandboxError) fail(sandboxError);
  }
  const chromiumArgs =
    process.platform === "linux" && !process.env.DISPLAY && !process.env.WAYLAND_DISPLAY
      ? ["--ozone-platform=headless", "--screen-info={8192x8192}"]
      : [];
  ensureDataDir();
  fs.mkdirSync(LOGS_DIR, { recursive: true });
  const quoted = [electron, main, "--daemon", ...chromiumArgs]
    .map((argument) => `'${argument.replaceAll("'", `'\\''`)}'`)
    .join(" ");
  const log = path.join(LOGS_DIR, "stderr.log").replaceAll("'", `'\\''`);
  return { command: ["/bin/sh", "-c", `exec ${quoted} 2>>'${log}'`], cwd: root };
}

function ownTtyPath(): string {
  try {
    const tty = execFileSync("tty", {
      stdio: ["inherit", "pipe", "ignore"],
      encoding: "utf8",
    }).trim();
    if (tty.startsWith("/dev/")) return tty;
  } catch {}
  fail("could not determine the current terminal");
}

function connectDaemon(): Promise<net.Socket> {
  return new Promise((resolve, reject) => {
    const socket = net.connect(DAEMON_SOCKET);
    socket.once("connect", () => resolve(socket));
    socket.once("error", reject);
  });
}

function spawnDaemon(): void {
  const { command, cwd } = browserLaunchCommand();
  const child = spawn(command[0], command.slice(1), {
    cwd,
    detached: true,
    stdio: "ignore",
  });
  child.unref();
}

async function daemonSocket(): Promise<net.Socket> {
  try {
    return await connectDaemon();
  } catch {}
  spawnDaemon();
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      return await connectDaemon();
    } catch {
      await sleep(200);
    }
  }
  throw new Error("renderer did not start");
}

interface DaemonReply {
  ok?: boolean;
  error?: string;
  session?: string;
  event?: string;
  code?: number;
}

function replies(socket: net.Socket, receive: (reply: DaemonReply) => void): void {
  let buffer = "";
  socket.on("data", (chunk) => {
    buffer += chunk.toString("utf8");
    let newline = buffer.indexOf("\n");
    while (newline !== -1) {
      const line = buffer.slice(0, newline);
      buffer = buffer.slice(newline + 1);
      newline = buffer.indexOf("\n");
      try {
        receive(JSON.parse(line) as DaemonReply);
      } catch {}
    }
  });
}

async function openSession(argv: string[], tty: string): Promise<net.Socket> {
  const socket = await daemonSocket();
  const reply = await new Promise<DaemonReply>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("renderer session timed out")), 20_000);
    replies(socket, (message) => {
      if (message.event) return;
      clearTimeout(timer);
      resolve(message);
    });
    socket.once("close", () => {
      clearTimeout(timer);
      reject(new Error("renderer closed before opening the editor"));
    });
    socket.write(
      `${JSON.stringify({
        cmd: "open",
        tty,
        argv,
        env: process.env,
        cwd: process.cwd(),
      })}\n`,
    );
  });
  if (reply.ok === false || !reply.session) {
    socket.destroy();
    throw new Error(reply.error ?? "renderer refused the editor session");
  }
  return socket;
}

async function main(): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) fail("an interactive terminal is required");
  const terminal = await checkTerminal(detect());
  if (terminal.graphics === "unsupported") {
    process.stderr.write(unsupportedGraphicsMessage(process.stderr.isTTY === true));
    process.exit(1);
  }
  const socket = await openSession(process.argv.slice(2), ownTtyPath());
  replies(socket, (message) => {
    if (message.event === "closed") process.exit(message.code ?? 0);
  });
  socket.on("close", () => process.exit(0));
  socket.on("error", () => process.exit(1));
  process.on("SIGWINCH", () => socket.write('{"cmd":"resize"}\n'));
  const close = () => {
    socket.write('{"cmd":"close"}\n');
    setTimeout(() => process.exit(0), 2000);
  };
  process.on("SIGINT", close);
  process.on("SIGTERM", close);
}

void main().catch((error: unknown) =>
  fail(error instanceof Error ? error.message : String(error)),
);
