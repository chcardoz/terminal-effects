import { BrowserWindow, screen } from "electron";
import type { EngineKeyEvent, PastedImage, PointerEvent, Surface, WheelEvent } from "pixel-react";

import { allowClipboardRead } from "./browser-session";
import { cursorShapeFor } from "./cursor";
import { frameRate } from "./frame-rate";
import { PageInput } from "./input";
import { offscreenPreferences } from "./offscreen";
import { BitmapPresenter, presentPaint, shmFrameOf } from "./paint";
import { PopupWindow } from "./popup";
import { cssSize } from "./types";
import type { BrowserSurfaceLayout } from "./types";
import { scaleZoom } from "./zoom";

interface BrowserControllerOptions {
  surface: Surface;
  popupSurface: Surface;
  layout: BrowserSurfaceLayout;
  url: string;
  background: string;
}

export class BrowserController {
  readonly surface: Surface;
  readonly input: PageInput;
  private readonly popupSurface: Surface;
  private readonly window: BrowserWindow;
  private layout: BrowserSurfaceLayout;
  private renderScale: number;
  private background: string;
  private stopped = false;
  private contentFocused = false;
  private wholeSurfaceNext = true;
  private readonly bitmaps: BitmapPresenter;
  private cdpAttached = false;
  private pendingPopupSize: { width: number; height: number } | null = null;
  private readonly popups: PopupWindow[] = [];
  cursorShape = "default";
  onCursorChange: (() => void) | null = null;
  onPopupChange: (() => void) | null = null;
  onClosed: (() => void) | null = null;

  private readonly onDisplayChange = () => {
    if (!this.stopped) this.window.webContents.setFrameRate(frameRate());
  };

  constructor(options: BrowserControllerOptions) {
    this.surface = options.surface;
    this.popupSurface = options.popupSurface;
    this.layout = options.layout;
    this.background = options.background;
    this.renderScale = browserRenderScale(options.layout);
    this.bitmaps = new BitmapPresenter(this.surface);
    const size = this.contentSize(options.layout);
    this.window = new BrowserWindow({
      width: size.width,
      height: size.height,
      useContentSize: true,
      show: false,
      frame: false,
      paintWhenInitiallyHidden: true,
      acceptFirstMouse: true,
      skipTaskbar: true,
      fullscreenable: false,
      resizable: false,
      webPreferences: {
        offscreen: offscreenPreferences(this.renderScale),
        sandbox: true,
        nodeIntegration: false,
        nodeIntegrationInSubFrames: true,
        contextIsolation: true,
        disableDialogs: true,
        backgroundThrottling: false,
      },
    });
    allowClipboardRead(this.window.webContents);
    this.input = new PageInput({
      contents: () => this.window.webContents,
      scale: () => this.layout.scale,
      focus: () => this.focusContent(),
      cdp: (method, params) => this.cdp(method, params),
    });
    this.window.webContents.setFrameRate(frameRate());
    this.window.on("closed", this.onWindowClosed);
    this.window.webContents.on("will-navigate", (event, url) => {
      if (this.quitLink(url)) event.preventDefault();
    });
    screen.on("display-added", this.onDisplayChange);
    screen.on("display-removed", this.onDisplayChange);
    screen.on("display-metrics-changed", this.onDisplayChange);
    this.window.webContents.on("paint", (event, dirtyRect, image) => {
      const sharedMemory = shmFrameOf(event);
      const presented =
        event.texture || sharedMemory
          ? presentPaint(
              this.surface,
              event.texture,
              sharedMemory,
              image,
              dirtyRect,
              this.wholeSurfaceNext,
            )
          : this.bitmaps.push(image, dirtyRect, this.wholeSurfaceNext);
      if (presented) this.wholeSurfaceNext = false;
    });
    this.window.webContents.on("cursor-changed", (_event, type) => {
      const shape = cursorShapeFor(type);
      if (shape === this.cursorShape) return;
      this.cursorShape = shape;
      this.onCursorChange?.();
    });
    this.window.webContents.setWindowOpenHandler((details) =>
      this.handleWindowOpen(details, this.window.webContents),
    );
    this.window.webContents.on("did-create-window", (child) => this.adoptPopup(child));
    void this.window.loadURL(options.url);
  }

  get popup(): PopupWindow | null {
    return this.popups[this.popups.length - 1] ?? null;
  }

  resize(layout: BrowserSurfaceLayout): void {
    if (this.stopped) return;
    if (
      this.layout.width === layout.width &&
      this.layout.height === layout.height &&
      this.layout.scale === layout.scale
    ) {
      return;
    }
    this.layout = layout;
    this.renderScale = browserRenderScale(layout);
    this.surface.clear();
    const size = this.contentSize(layout);
    this.window.setContentSize(size.width, size.height, false);
  }

  pointer(event: PointerEvent): void {
    if (!this.stopped) this.input.pointer(event);
  }

  wheel(event: WheelEvent): void {
    if (!this.stopped) this.input.wheel(event);
  }

  key(event: EngineKeyEvent): void {
    if (!this.stopped) this.input.key(event);
  }

  paste(text: string): void {
    this.input.paste(text);
  }

  pasteImage(image: PastedImage): void {
    this.input.pasteImage(image);
  }

  selectionText(): Promise<string> {
    return this.input.selectionText();
  }

  setActive(active: boolean): void {
    if (!active) this.blurContent();
  }

  scaleZoom(ratio: number): number {
    return scaleZoom(this.window.webContents, ratio);
  }

  async setBackground(background: string): Promise<void> {
    this.background = background;
    await this.emulateColorScheme();
  }

  sendToPage(channel: string, payload: unknown): void {
    try {
      this.window.webContents.send(channel, payload);
    } catch {}
  }

  hasContents(id: number): boolean {
    return this.window.webContents.id === id;
  }

  stop(): void {
    if (this.stopped) return;
    this.stopped = true;
    this.teardown();
    this.window.destroy();
  }

  private focusContent(): Promise<void> | undefined {
    if (this.stopped || this.contentFocused) return;
    this.window.focus();
    this.window.webContents.focus();
    this.contentFocused = true;
    return this.cdp("Emulation.setFocusEmulationEnabled", { enabled: true }).then(
      () => undefined,
      () => undefined,
    );
  }

  private blurContent(): void {
    if (!this.contentFocused) return;
    this.input.releaseKeys();
    this.window.blurWebView();
    this.contentFocused = false;
    void this.cdp("Emulation.setFocusEmulationEnabled", { enabled: false }).catch(() => {});
  }

  private async attachCdp(): Promise<void> {
    if (this.cdpAttached) return;
    this.window.webContents.debugger.attach("1.3");
    this.cdpAttached = true;
    await this.emulateColorScheme();
  }

  private async cdp(method: string, params?: Record<string, unknown>): Promise<unknown> {
    await this.attachCdp();
    return this.window.webContents.debugger.sendCommand(method, params);
  }

  private async emulateColorScheme(): Promise<void> {
    if (!this.cdpAttached) return;
    const value = Number.parseInt(this.background.slice(1), 16);
    const red = (value >> 16) & 255;
    const green = (value >> 8) & 255;
    const blue = value & 255;
    const dark = 0.2126 * red + 0.7152 * green + 0.0722 * blue < 128;
    await this.window.webContents.debugger.sendCommand("Emulation.setEmulatedMedia", {
      features: [{ name: "prefers-color-scheme", value: dark ? "dark" : "light" }],
    });
  }

  private teardown(): void {
    for (const popup of [...this.popups]) popup.close();
    screen.off("display-added", this.onDisplayChange);
    screen.off("display-removed", this.onDisplayChange);
    screen.off("display-metrics-changed", this.onDisplayChange);
    this.surface.close();
  }

  private readonly onWindowClosed = () => {
    if (this.stopped) return;
    this.stopped = true;
    this.teardown();
    this.onClosed?.();
  };

  private contentSize(layout: BrowserSurfaceLayout) {
    return cssSize(layout.width, layout.height, layout.scale);
  }

  private quitLink(url: string): boolean {
    if (!url.startsWith("terminal-effects-renderer://quit")) return false;
    setImmediate(() => {
      if (!this.stopped) this.window.close();
    });
    return true;
  }

  private handleWindowOpen(
    { url, disposition, features }: Electron.HandlerDetails,
    opener: Electron.WebContents,
  ): Electron.WindowOpenHandlerResponse {
    if (this.quitLink(url)) return { action: "deny" };
    const popupDisposition =
      disposition === "new-window" ||
      disposition === "foreground-tab" ||
      disposition === "background-tab";
    if (popupDisposition) {
      const size = this.popupSize(features, disposition !== "new-window");
      this.pendingPopupSize = size;
      return {
        action: "allow",
        overrideBrowserWindowOptions: {
          width: size.width,
          height: size.height,
          useContentSize: true,
          show: false,
          frame: false,
          skipTaskbar: true,
          fullscreenable: false,
          resizable: false,
          webPreferences: {
            offscreen: { useSharedTexture: false, deviceScaleFactor: this.renderScale },
            sandbox: true,
            nodeIntegration: false,
            nodeIntegrationInSubFrames: true,
            contextIsolation: true,
            disableDialogs: true,
            backgroundThrottling: false,
          },
        },
      };
    }
    void opener.loadURL(url);
    return { action: "deny" };
  }

  private adoptPopup(child: BrowserWindow): void {
    allowClipboardRead(child.webContents);
    child.webContents.on("did-create-window", (grandchild) => this.adoptPopup(grandchild));
    const size = this.pendingPopupSize ?? { width: 480, height: 360 };
    this.pendingPopupSize = null;
    const popup = new PopupWindow(
      child,
      this.popupSurface,
      size,
      this.renderScale,
      () => this.layout.scale,
      () => this.onPopupChange?.(),
      () => {
        const index = this.popups.indexOf(popup);
        if (index < 0) return;
        const wasTop = index === this.popups.length - 1;
        this.popups.splice(index, 1);
        if (wasTop) this.popup?.setVisible(true);
        this.onPopupChange?.();
      },
      (details) => this.handleWindowOpen(details, child.webContents),
    );
    popup.onCursorChange = () => {
      if (this.popup === popup) this.onCursorChange?.();
    };
    this.popup?.setVisible(false);
    this.popups.push(popup);
    this.onPopupChange?.();
  }

  private popupSize(features: string, tab: boolean): { width: number; height: number } {
    const content = this.contentSize(this.layout);
    if (tab) {
      return {
        width: Math.max(280, Math.round(content.width * 0.9)),
        height: Math.max(280, Math.round(content.height * 0.9)),
      };
    }
    const requested = (name: string) => {
      const match = features.match(new RegExp(`${name}=(\\d+)`));
      return match ? Number(match[1]) : 0;
    };
    const clamp = (value: number, fallback: number, max: number) =>
      Math.max(280, Math.min(value || fallback, max));
    return {
      width: clamp(
        requested("width"),
        Math.round(content.width * 0.62),
        Math.round(content.width * 0.85),
      ),
      height: clamp(
        requested("height"),
        Math.round(content.height * 0.68),
        Math.round(content.height * 0.8),
      ),
    };
  }
}

function browserRenderScale(layout: BrowserSurfaceLayout): number {
  const explicit = Number(process.env.TE_RENDERER_RENDER_SCALE);
  if (Number.isFinite(explicit) && explicit > 0) {
    return Math.max(0.5, Math.min(layout.scale, explicit));
  }
  const maxPixels = Number(process.env.TE_RENDERER_MAX_PIXELS ?? 0);
  if (!Number.isFinite(maxPixels) || maxPixels <= 0) return layout.scale;
  const cssPixels = (layout.width * layout.height) / (layout.scale * layout.scale);
  return Math.max(0.5, Math.min(layout.scale, Math.sqrt(maxPixels / cssPixels)));
}
