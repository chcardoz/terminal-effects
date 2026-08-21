import type { DamageRect, NativeEngine, SurfaceShm } from "./native";

type Damaged = { damage?: DamageRect };

export type SurfaceTexture = { ioSurface: Buffer } | { shm: SurfaceShm };

export type SurfaceFrame =
  | ({ bgra: Buffer; width: number; height: number } & Damaged) // i think we want to kilt the case of passing the raw pixels to react, there's never a case that I know of
  | (SurfaceTexture & Damaged & Released);

interface Released {
  released?: () => void;
}

export class Surface {
  private readonly id: number;
  private readonly engine: NativeEngine;
  private closed = false;

  constructor(engine: NativeEngine, id: number) {
    this.engine = engine;
    this.id = id;
  }

  present(frame: SurfaceFrame): void {
    if (this.closed) {
      if ("shm" in frame) frame.released?.();
      return;
    }
    if ("ioSurface" in frame) {
      const submit = this.engine.updateSurfaceTexture;
      if (!submit) throw new Error("IOSurface frames are not supported on this platform");
      submit.call(this.engine, this.id, frame.ioSurface, frame.damage);
    } else if ("shm" in frame) {
      const submit = this.engine.updateSurfaceShm;
      if (!submit) throw new Error("shared memory frames are not supported on this platform");
      submit.call(this.engine, this.id, frame.shm, frame.damage, frame.released);
    } else {
      this.engine.updateSurface(this.id, frame.bgra, frame.width, frame.height, frame.damage);
    }
  }

  clear(): void {
    if (this.closed) return;
    this.engine.removeSurface(this.id);
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.engine.removeSurface(this.id);
  }
}

export function surfaceId(surface: Surface): number {
  return (surface as unknown as { id: number }).id;
}
