export interface DamageRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SurfaceShm {
  fd: number;
  width: number;
  height: number;
  stride: number;
  size: number;
}

export interface NativeEngine {
  info(): string;
  applyOps(ops: string): void;
  updateSurface(
    id: number,
    bgra: Buffer,
    width: number,
    height: number,
    damage?: DamageRect,
  ): void;
  updateSurfaceTexture?(id: number, handle: Buffer, damage?: DamageRect): void;
  updateSurfaceShm?(
    id: number,
    shm: SurfaceShm,
    damage?: DamageRect,
    released?: (...args: unknown[]) => void,
  ): void;
  removeSurface(id: number): void;
  surfaceStats(): string;
  setKeyEventTypes(enabled: boolean): void;
  start(callback: (err: unknown, event: string) => void): void;
  stop(): void;
}

export type Rgba = [number, number, number, number];

export type TerminalColors = {
  foreground: Rgba | null;
  background: Rgba | null;
  palette: (Rgba | null)[];
};

/**
 * fixme: this is a very weird name to export
 */
export interface EngineInfo {
  width: number;
  height: number;
  cellWidth: number;
  cellHeight: number;
  basePx: number;
  kittyKeyboard: boolean;
  colors: TerminalColors;
}

// eslint-disable-next-line @typescript-eslint/no-var-requires
const binding = require("../native/pixel.node") as {
  PixelEngine: new (
    tty?: string,
    wrapper?: string,
    sessionEnv?: Record<string, string>,
  ) => NativeEngine;
};

export function createNativeEngine(
  tty?: string,
  wrapper?: string,
  sessionEnv?: NodeJS.ProcessEnv,
): NativeEngine {
  const env = sessionEnv
    ? Object.fromEntries(
        Object.entries(sessionEnv).filter(
          (entry): entry is [string, string] => typeof entry[1] === "string",
        ),
      )
    : undefined;
  const pixelEngine = new binding.PixelEngine(tty, wrapper, env);

  return pixelEngine
}
