import { create } from "zustand";
import { request } from "../lib/api";
import { clamp } from "../lib/time";
import type { EditTool, LeftPanel, ProjectPayload, RightPanel } from "../types";

interface EditorState {
  data: ProjectPayload | null;
  loading: boolean;
  saving: boolean;
  playing: boolean;
  muted: boolean;
  snapping: boolean;
  interacting: boolean;
  zoom: number;
  leftPanel: LeftPanel;
  rightPanel: RightPanel;
  editTool: EditTool;
  status: string;
  toast: { message: string; error: boolean } | null;
  importOpen: boolean;
  loadProject(silent?: boolean): Promise<void>;
  runAction(path: string, body: unknown, success: string): Promise<boolean>;
  setFrame(frame: number, persist?: boolean): void;
  persistFrame(immediate?: boolean): void;
  selectClip(clipId: string): void;
  setPlaying(playing: boolean): void;
  setMuted(muted: boolean): void;
  setSnapping(snapping: boolean): void;
  setInteracting(interacting: boolean): void;
  setZoom(zoom: number): void;
  setLeftPanel(panel: LeftPanel): void;
  setRightPanel(panel: RightPanel): void;
  setEditTool(tool: EditTool): void;
  setStatus(status: string): void;
  showToast(message: string, error?: boolean): void;
  setImportOpen(open: boolean): void;
}

let playheadTimer = 0;
let toastTimer = 0;

export const useEditorStore = create<EditorState>((set, get) => ({
  data: null,
  loading: true,
  saving: false,
  playing: false,
  muted: false,
  snapping: true,
  interacting: false,
  zoom: 72,
  leftPanel: "media",
  rightPanel: "properties",
  editTool: "select",
  status: "Opening project…",
  toast: null,
  importOpen: false,

  async loadProject(silent = false) {
    try {
      const payload = await request<ProjectPayload>("api/project");
      set({ data: payload, loading: false, ...(silent ? {} : { status: "Ready" }) });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ loading: false, status: message });
      get().showToast(message, true);
    }
  },

  async runAction(path, body, success) {
    set({ saving: true, status: success === "Export complete" ? "Exporting with FFmpeg…" : "Saving…" });
    try {
      await request(path, body);
      await get().loadProject(true);
      set({ status: success });
      return true;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ status: message });
      get().showToast(message, true);
      return false;
    } finally {
      set({ saving: false });
    }
  },

  setFrame(frame, persist = true) {
    set((state) => {
      if (!state.data) return state;
      const next = clamp(Math.round(frame), 0, state.data.durationFrames);
      return { data: { ...state.data, project: { ...state.data.project, playheadFrame: next } } };
    });
    if (persist) get().persistFrame(false);
  },

  persistFrame(immediate = false) {
    window.clearTimeout(playheadTimer);
    const save = () => {
      const frame = get().data?.project.playheadFrame;
      if (frame !== undefined) void request("api/playhead", { frame }).catch((error: Error) => get().showToast(error.message, true));
    };
    if (immediate) save();
    else playheadTimer = window.setTimeout(save, 150);
  },

  selectClip(clipId) {
    set((state) => state.data ? {
      data: { ...state.data, project: { ...state.data.project, selectedClipId: clipId } },
      rightPanel: "properties",
    } : state);
    void request("api/select", { clipId }).catch((error: Error) => get().showToast(error.message, true));
  },

  setPlaying: (playing) => set({ playing }),
  setMuted: (muted) => set({ muted }),
  setSnapping: (snapping) => set({ snapping }),
  setInteracting: (interacting) => set({ interacting }),
  setZoom: (zoom) => set({ zoom }),
  setLeftPanel: (leftPanel) => set({ leftPanel }),
  setRightPanel: (rightPanel) => set({ rightPanel }),
  setEditTool: (editTool) => set({ editTool, status: editTool === "blade" ? "Blade tool — click a clip to split" : "Selection tool" }),
  setStatus: (status) => set({ status }),
  showToast(message, error = false) {
    window.clearTimeout(toastTimer);
    set({ toast: { message, error } });
    toastTimer = window.setTimeout(() => set({ toast: null }), 2800);
  },
  setImportOpen: (importOpen) => set({ importOpen }),
}));

export function selectedClip() {
  const data = useEditorStore.getState().data;
  return data?.project.clips.find((clip) => clip.id === data.project.selectedClipId);
}
