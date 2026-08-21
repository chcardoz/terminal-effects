import { Clapperboard, Download, Redo2, Undo2 } from "lucide-react";
import { useEditorStore } from "../store/editor";
import { IconButton } from "./IconButton";

export function TopBar() {
  const name = useEditorStore((state) => state.data?.project.name ?? "Opening project");
  const root = useEditorStore((state) => state.data?.root ?? "");
  const saving = useEditorStore((state) => state.saving);
  const runAction = useEditorStore((state) => state.runAction);

  return (
    <header className="topbar">
      <div className="app-identity">
        <span className="app-icon"><Clapperboard size={17} strokeWidth={1.9} /></span>
        <div className="project-identity">
          <strong>{name}</strong>
          <span title={root}>{root}</span>
        </div>
      </div>

      <div className="topbar-history" aria-label="History">
        <IconButton label="Undo (⌘Z)" data-testid="undo" onClick={() => void runAction("api/undo", {}, "Undo")}>
          <Undo2 size={17} />
        </IconButton>
        <IconButton label="Redo (⇧⌘Z)" data-testid="redo" onClick={() => void runAction("api/redo", {}, "Redo")}>
          <Redo2 size={17} />
        </IconButton>
      </div>

      <div className="topbar-actions">
        <span className={`save-state ${saving ? "is-saving" : ""}`}>
          <i />{saving ? "Saving" : "Saved"}
        </span>
        <button
          className="export-button"
          data-testid="export"
          disabled={saving}
          onClick={() => void runAction("api/export", {}, "Export complete")}
        >
          <Download size={15} />
          Export
        </button>
      </div>
    </header>
  );
}
