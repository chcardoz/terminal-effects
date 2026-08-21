import { Import, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useEditorStore } from "../store/editor";
import { IconButton } from "./IconButton";

export function ImportDialog() {
  const open = useEditorStore((state) => state.importOpen);
  const setOpen = useEditorStore((state) => state.setImportOpen);
  const runAction = useEditorStore((state) => state.runAction);
  const [paths, setPaths] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => { if (open) input.current?.focus(); }, [open]);
  if (!open) return null;
  return (
    <div className="dialog-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) setOpen(false); }}>
      <form className="dialog" role="dialog" aria-modal="true" aria-labelledby="import-title" onSubmit={async (event) => {
        event.preventDefault();
        const mediaPaths = paths.split("\n").map((path) => path.trim()).filter(Boolean);
        if (!mediaPaths.length) return;
        setOpen(false);
        if (await runAction("api/import", { paths: mediaPaths }, `Imported ${mediaPaths.length} file${mediaPaths.length === 1 ? "" : "s"}`)) setPaths("");
      }}>
        <header><div className="dialog-icon"><Import size={19} /></div><div><h2 id="import-title">Import media</h2><p>Add one absolute media path per line.</p></div><IconButton label="Close" onClick={() => setOpen(false)}><X size={17} /></IconButton></header>
        <textarea ref={input} value={paths} onChange={(event) => setPaths(event.target.value)} rows={6} placeholder="/Users/you/Videos/clip.mp4" />
        <footer><button type="button" className="quiet-button" onClick={() => setOpen(false)}>Cancel</button><button type="submit" className="primary-button">Import files</button></footer>
      </form>
    </div>
  );
}
