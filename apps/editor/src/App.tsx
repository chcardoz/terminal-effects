import { useEffect } from "react";
import { ImportDialog } from "./components/ImportDialog";
import { Inspector } from "./components/Inspector";
import { LeftToolRail, RightToolRail } from "./components/ToolRails";
import { MediaDrawer } from "./components/MediaDrawer";
import { PreviewMonitor } from "./components/PreviewMonitor";
import { Timeline } from "./components/Timeline";
import { TopBar } from "./components/TopBar";
import { useEditorStore } from "./store/editor";

export default function App() {
  const loading = useEditorStore((state) => state.loading);
  const toast = useEditorStore((state) => state.toast);

  useEffect(() => {
    void useEditorStore.getState().loadProject();
    const polling = window.setInterval(() => {
      const state = useEditorStore.getState();
      if (!state.playing && !state.interacting && document.visibilityState === "visible") void state.loadProject(true);
    }, 900);
    return () => window.clearInterval(polling);
  }, []);

  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (target.matches("input, textarea")) return;
      const state = useEditorStore.getState();
      const project = state.data?.project;
      const command = event.metaKey || event.ctrlKey;
      if (command && event.key.toLowerCase() === "z") {
        event.preventDefault();
        void state.runAction(event.shiftKey ? "api/redo" : "api/undo", {}, event.shiftKey ? "Redo" : "Undo");
      } else if (event.code === "Space") {
        event.preventDefault();
        (document.querySelector('[data-testid="play"]') as HTMLButtonElement | null)?.click();
      } else if (event.key === "ArrowLeft" && project) {
        event.preventDefault(); state.setFrame(project.playheadFrame - (event.shiftKey ? Math.round(state.data!.fpsValue) : 1));
      } else if (event.key === "ArrowRight" && project) {
        event.preventDefault(); state.setFrame(project.playheadFrame + (event.shiftKey ? Math.round(state.data!.fpsValue) : 1));
      } else if (event.key.toLowerCase() === "b") {
        state.setEditTool("blade");
      } else if (event.key.toLowerCase() === "v") {
        state.setEditTool("select");
      } else if (event.key.toLowerCase() === "s" && project?.selectedClipId) {
        void state.runAction("api/split", { clipId: project.selectedClipId, frame: project.playheadFrame }, "Clip split");
      } else if ((event.key === "Backspace" || event.key === "Delete") && project?.selectedClipId) {
        event.preventDefault(); void state.runAction("api/remove", { clipId: project.selectedClipId }, "Clip removed");
      } else if (event.key.toLowerCase() === "q" && window.terminalEffectsHost?.quit) {
        window.terminalEffectsHost.quit();
      }
    };
    document.addEventListener("keydown", keyboard);
    return () => document.removeEventListener("keydown", keyboard);
  }, []);

  return (
    <div className="editor-app" aria-busy={loading}>
      <TopBar />
      <main className="editor-workspace">
        <LeftToolRail />
        <MediaDrawer />
        <PreviewMonitor />
        <Timeline />
        <Inspector />
        <RightToolRail />
      </main>
      {loading && <div className="app-loading"><span /><strong>Opening editor</strong></div>}
      <ImportDialog />
      {toast && <div className={`toast ${toast.error ? "is-error" : ""}`} role="status">{toast.message}</div>}
    </div>
  );
}
