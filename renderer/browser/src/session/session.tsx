import fs from "node:fs";
import path from "node:path";

import { app, ipcMain, screen } from "electron";
import type { IpcMainEvent, Session as ElectronSession } from "electron";
import { createRoot } from "pixel-react";
import type { EngineKeyEvent, PixelRoot, PastedImage, Surface } from "pixel-react";
import { detect } from "pixel-terminals";
import type { Terminal } from "pixel-terminals";

import { BrowserController } from "../page/controller";
import { browserSession, configureBrowserSession } from "../page/browser-session";
import { initOffscreenMode } from "../page/offscreen";
import { snapToCssGrid } from "../page/types";
import type { BrowserSurfaceLayout } from "../page/types";
import { AppView } from "../ui/app";
import type { AppLayout, PopupView } from "../ui/app";

export interface SessionContext {
  tty: string;
  argv: string[];
  env: NodeJS.ProcessEnv;
  cwd: string;
  onClose(code: number): void;
}

export interface SessionHandle {
  ready: Promise<void>;
  close(code?: number): void;
  nudgeResize(): void;
}

export function createSession(ctx: SessionContext): SessionHandle {
  const session = new Session(ctx);
  const ready = session.start().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
    session.shutdown(1);
  });
  return {
    ready,
    close: (code = 0) => session.shutdown(code),
    nudgeResize: () => session.nudgeResize(),
  };
}

const FONT_FILE = path.join("assets", "fonts", "JetBrainsMono-Regular.ttf");

const API_PRELOAD_SOURCE = `if (process.isMainFrame) {
  const { ipcRenderer } = require("electron");
  let current = null;
  const subscribers = new Set();
  ipcRenderer.on("terminal-effects-renderer:theme", (_event, theme) => {
    current = theme;
    for (const subscriber of subscribers) {
      try { subscriber(theme); } catch {}
    }
  });
  ipcRenderer.send("terminal-effects-renderer:theme-request");
  globalThis.terminalEffectsRenderer = {
    theme: () => current,
    onTheme(subscriber) {
      subscribers.add(subscriber);
      if (current) { try { subscriber(current); } catch {} }
      return () => subscribers.delete(subscriber);
    },
    quit: () => ipcRenderer.send("terminal-effects-renderer:quit"),
  };
}
`;

let apiPreloadFile: string | null = null;
function apiPreloadPath(): string {
  if (!apiPreloadFile) {
    apiPreloadFile = path.join(app.getPath("userData"), "terminal-effects-renderer-api-preload.js");
    fs.writeFileSync(apiPreloadFile, API_PRELOAD_SOURCE);
  }
  return apiPreloadFile;
}

const registeredPreloads = new WeakMap<ElectronSession, Set<string>>();
function registerPreloadOnce(session: ElectronSession, filePath: string): void {
  let seen = registeredPreloads.get(session);
  if (!seen) registeredPreloads.set(session, (seen = new Set()));
  if (seen.has(filePath)) return;
  seen.add(filePath);
  session.registerPreloadScript({ type: "frame", filePath });
}

function bundledFontPath(): string {
  for (let dir = __dirname; ; dir = path.dirname(dir)) {
    const candidate = path.join(dir, FONT_FILE);
    if (fs.existsSync(candidate)) return candidate;
    if (path.dirname(dir) === dir) {
      throw new Error(`bundled font missing: ${FONT_FILE} (searched up from ${__dirname})`);
    }
  }
}

class Session {
  private readonly ctx: SessionContext;
  private readonly terminal: Terminal | null;
  private readonly initialUrl: string;
  private readonly preload: string | null;
  private root: PixelRoot | null = null;
  private page: BrowserController | null = null;
  private popupSurface: Surface | null = null;
  private layout: AppLayout | null = null;
  private surfaceLayout: BrowserSurfaceLayout | null = null;
  private displayScale = 1;
  private fontId = 0;
  private windowBg = "#1e2026";
  private cellFollow: { height: number; basePx: number } | null = null;
  private shuttingDown = false;

  private readonly onThemeRequest = (event: IpcMainEvent) => {
    if (this.ownsSender(event)) event.sender.send("terminal-effects-renderer:theme", this.themePayload());
  };

  private readonly onQuitRequest = (event: IpcMainEvent) => {
    if (this.ownsSender(event)) this.shutdown();
  };

  constructor(ctx: SessionContext) {
    this.ctx = ctx;
    this.terminal = detect(ctx.env);
    const url = ctx.argv.find((argument) => !argument.startsWith("-"));
    if (!url) throw new Error("Terminal Effects renderer needs an editor URL");
    this.initialUrl = url;
    this.preload = flagValue(ctx.argv, "--preload");
  }

  async start(): Promise<void> {
    if (process.platform === "darwin") app.dock?.hide();
    configureBrowserSession();
    this.installEmbedderApi();
    this.displayScale = this.hostDisplayScale();
    this.root = createRoot({
      tty: this.ctx.tty,
      wrapper: this.terminal?.wrapper,
      sessionEnv: this.ctx.env,
      keyEventTypes: true,
      onKey: (event) => this.handleKey(event),
      onPaste: (text) => this.inputTarget()?.paste(text),
      onPasteImage: (image) => this.pasteImage(image),
      onFocus: (focused) => this.page?.setActive(focused),
      onResize: () => {
        this.followCellZoom();
        this.recalculateLayout();
        if (this.surfaceLayout) this.page?.resize(this.surfaceLayout);
        this.render();
      },
      onColors: () => {
        this.windowBg = this.themeBackground();
        void this.page?.setBackground(this.windowBg);
        this.render();
        this.broadcastTheme();
      },
      onEngineExit: (error) => {
        if (error) process.stderr.write(`Terminal Effects renderer: ${error}\n`);
        this.shutdown(error ? 1 : 0);
      },
    });
    initOffscreenMode(this.root.sharedTextures);
    this.fontId = await this.root.registerFont(bundledFontPath());
    this.popupSurface = this.root.createSurface();
    this.followCellZoom();
    this.recalculateLayout();
    this.root.setPointerShape("default");
    this.windowBg = this.themeBackground();
    this.page = new BrowserController({
      surface: this.root.createSurface(),
      popupSurface: this.popupSurface,
      layout: this.surfaceLayout!,
      url: this.initialUrl,
      background: this.windowBg,
    });
    this.page.onPopupChange = () => {
      this.syncCursor();
      this.render();
    };
    this.page.onCursorChange = () => this.syncCursor();
    this.page.onClosed = () => this.shutdown();
    this.render();
    this.broadcastTheme();
  }

  nudgeResize(): void {
    this.root?.nudgeResize();
  }

  shutdown(code = 0): void {
    if (this.shuttingDown) return;
    this.shuttingDown = true;
    ipcMain.off("terminal-effects-renderer:theme-request", this.onThemeRequest);
    ipcMain.off("terminal-effects-renderer:quit", this.onQuitRequest);
    this.page?.stop();
    this.page = null;
    this.popupSurface?.close();
    this.popupSurface = null;
    this.root?.stop();
    this.root = null;
    this.ctx.onClose(code);
  }

  private installEmbedderApi(): void {
    if (!this.preload) return;
    ipcMain.on("terminal-effects-renderer:theme-request", this.onThemeRequest);
    ipcMain.on("terminal-effects-renderer:quit", this.onQuitRequest);
    const session = browserSession();
    registerPreloadOnce(session, apiPreloadPath());
    registerPreloadOnce(session, path.resolve(this.ctx.cwd, this.preload));
  }

  private ownsSender(event: IpcMainEvent): boolean {
    return event.senderFrame === event.sender.mainFrame && this.page?.hasContents(event.sender.id) === true;
  }

  private themePayload(): {
    background: number[];
    foreground: number[];
    ansi: (number[] | null)[];
  } | null {
    if (!this.root) return null;
    const colors = this.root.info.colors;
    if (!colors.background || !colors.foreground) return null;
    const rgb = (channels: number[] | null) =>
      channels ? [channels[0], channels[1], channels[2]] : null;
    return {
      background: rgb(colors.background) as number[],
      foreground: rgb(colors.foreground) as number[],
      ansi: Array.from({ length: 16 }, (_, index) => rgb(colors.palette[index] ?? null)),
    };
  }

  private broadcastTheme(): void {
    if (!this.preload) return;
    const payload = this.themePayload();
    if (payload) this.page?.sendToPage("terminal-effects-renderer:theme", payload);
  }

  private inputTarget() {
    return this.page?.popup?.input ?? this.page;
  }

  private pasteImage(image: PastedImage): void {
    this.inputTarget()?.pasteImage(image);
  }

  private handleKey(event: EngineKeyEvent): void {
    const popup = this.page?.popup;
    if (popup && event.kind !== "release" && event.key === "escape") {
      popup.close();
      return;
    }
    const input = this.inputTarget();
    if (!input) return;
    if (event.kind !== "release") {
      if (isPasteKey(event)) this.root?.requestClipboardImage();
      if (isCopyKey(event) || isCutKey(event)) void this.copySelection();
    }
    input.key(event);
  }

  private async copySelection(): Promise<void> {
    const text = await this.inputTarget()?.selectionText();
    if (text) this.root?.setClipboard(text);
  }

  private recalculateLayout(): void {
    if (!this.root) return;
    const snapped = snapToCssGrid(this.root.info.width, this.root.info.height, this.displayScale);
    this.surfaceLayout = { x: 0, y: 0, ...snapped, scale: this.displayScale };
    this.layout = {
      width: this.root.info.width,
      height: this.root.info.height,
      rem: this.root.info.basePx,
      page: { x: 0, y: 0, width: snapped.width, height: snapped.height },
    };
  }

  private popupView(): PopupView | null {
    const popup = this.page?.popup;
    if (!popup || !this.layout || !this.surfaceLayout) return null;
    const scale = this.surfaceLayout.scale;
    const header = Math.round(this.layout.rem * 1.7);
    const maxWidth = Math.round(this.layout.page.width * 0.94);
    const maxHeight = Math.round(this.layout.page.height * 0.94) - header;
    let host = "";
    try {
      host = new URL(popup.state.url).host;
    } catch {}
    return {
      title: popup.state.title,
      host,
      loading: popup.state.loading,
      width: Math.max(60, Math.min(Math.round(popup.state.width * scale), maxWidth)),
      height: Math.max(60, Math.min(Math.round(popup.state.height * scale), maxHeight)),
    };
  }

  private render(): void {
    if (!this.root || !this.page || !this.popupSurface || !this.layout) return;
    this.root.render(
      <AppView
        layout={this.layout}
        colors={this.root.info.colors}
        font={this.fontId}
        pageSurface={this.page.surface}
        popupSurface={this.popupSurface}
        popup={this.popupView()}
        onPagePointer={(event) => this.page?.pointer(event)}
        onPageWheel={(event) => this.page?.wheel(event)}
        onPopupPointer={(event) => this.page?.popup?.input.pointer(event)}
        onPopupWheel={(event) => this.page?.popup?.input.wheel(event)}
        onPopupClose={() => this.page?.popup?.close()}
      />,
    );
  }

  private syncCursor(): void {
    const shape = this.page?.popup?.cursorShape ?? this.page?.cursorShape ?? "default";
    this.root?.setPointerShape(shape);
  }

  private followCellZoom(): void {
    if (!this.root) return;
    const { height, basePx } = this.root.info;
    const previous = this.cellFollow;
    this.cellFollow = { height, basePx };
    if (!previous?.basePx || !previous.height) return;
    const ratio = basePx / previous.basePx;
    if (!Number.isFinite(ratio) || ratio <= 0 || Math.abs(ratio - 1) < 0.01) return;
    const paneRatio = height / previous.height;
    if (Math.abs(paneRatio - ratio) < 0.04 * ratio) return;
    this.page?.scaleZoom(ratio);
    this.page?.popup?.scaleZoom(ratio);
  }

  private hostDisplayScale(): number {
    const explicit = Number(this.ctx.env.TE_RENDERER_DISPLAY_SCALE);
    if (Number.isFinite(explicit) && explicit > 0) return explicit;
    if (this.terminal?.reportsCssPixels) return 1;
    return screen.getDisplayNearestPoint(screen.getCursorScreenPoint()).scaleFactor;
  }

  private themeBackground(): string {
    const background = this.root?.info.colors.background ?? [30, 32, 38, 255];
    return `#${background
      .slice(0, 3)
      .map((channel) => channel.toString(16).padStart(2, "0"))
      .join("")}`;
  }
}

function flagValue(argv: string[], flag: string): string | null {
  return argv.find((argument) => argument.startsWith(`${flag}=`))?.slice(flag.length + 1) ?? null;
}

function acceleratorHeld(event: EngineKeyEvent): boolean {
  return process.platform === "darwin" ? event.mods.super : event.mods.ctrl;
}

function isPasteKey(event: EngineKeyEvent): boolean {
  return acceleratorHeld(event) && event.key === "v";
}

function isCopyKey(event: EngineKeyEvent): boolean {
  return acceleratorHeld(event) && event.key === "c";
}

function isCutKey(event: EngineKeyEvent): boolean {
  return acceleratorHeld(event) && event.key === "x";
}
