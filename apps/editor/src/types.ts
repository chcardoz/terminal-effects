export type AssetKind = "video" | "audio";
export type TrackKind = "video" | "audio";
export type FitMode = "contain" | "cover";

export interface ClipTransform {
  rotationDegrees: 0 | 90 | 180 | 270;
  fit: FitMode;
  positionX: number;
  positionY: number;
}

export interface Fps {
  numerator: number;
  denominator: number;
}

export interface Asset {
  id: string;
  name: string;
  path: string;
  kind: AssetKind;
  durationFrames: number;
  width: number;
  height: number;
  hasAudio: boolean;
  mediaUrl: string;
  thumbnailUrl: string;
}

export interface Clip {
  id: string;
  assetId: string;
  trackId: string;
  startFrame: number;
  durationFrames: number;
  sourceInFrame: number;
  filmstripUrl: string;
  waveformUrl: string;
  transform: ClipTransform;
}

export interface Track {
  id: string;
  kind: TrackKind;
  name: string;
}

export interface Project {
  schemaVersion: number;
  name: string;
  revision: number;
  fps: Fps;
  width: number;
  height: number;
  playheadFrame: number;
  selectedClipId: string | null;
  assets: Asset[];
  tracks: Track[];
  clips: Clip[];
}

export interface ProjectPayload {
  project: Project;
  root: string;
  projectFile: string;
  durationFrames: number;
  fpsValue: number;
  renderer: "chromium-offscreen";
}

export type LeftPanel = "media" | "audio" | "text" | "transitions";
export type RightPanel = "properties" | "audio" | "color" | null;
export type EditTool = "select" | "blade";

declare global {
  interface Window {
    terminalEffectsHost?: {
      quit(): void;
      theme(): unknown;
    };
  }
}
