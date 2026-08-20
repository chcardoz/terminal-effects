import { Film, Import, List, Music2, Plus, Search, Video } from "lucide-react";
import { useMemo, useState } from "react";
import { formatDuration } from "../lib/time";
import { useEditorStore } from "../store/editor";
import type { Asset } from "../types";
import { IconButton } from "./IconButton";

export function MediaDrawer() {
  const data = useEditorStore((state) => state.data);
  const leftPanel = useEditorStore((state) => state.leftPanel);
  const selectClip = useEditorStore((state) => state.selectClip);
  const setFrame = useEditorStore((state) => state.setFrame);
  const setImportOpen = useEditorStore((state) => state.setImportOpen);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<"grid" | "list">("grid");

  const assets = useMemo(() => {
    const source = data?.project.assets ?? [];
    return source.filter((asset) => {
      const kindMatches = leftPanel === "media" || (leftPanel === "audio" && (asset.kind === "audio" || asset.hasAudio));
      return kindMatches && asset.name.toLowerCase().includes(query.toLowerCase());
    });
  }, [data, leftPanel, query]);

  const choose = (asset: Asset, seek: boolean) => {
    const clip = data?.project.clips.find((candidate) => candidate.assetId === asset.id);
    if (!clip) return;
    selectClip(clip.id);
    if (seek) setFrame(clip.startFrame);
  };

  const fps = data?.fpsValue ?? 30;
  const title = leftPanel === "audio" ? "Audio" : "Your media";

  return (
    <aside className="content-drawer">
      <header className="drawer-header">
        <div><h2>{title}</h2><span>{assets.length} item{assets.length === 1 ? "" : "s"}</span></div>
        <IconButton label="Import media" onClick={() => setImportOpen(true)}><Plus size={18} /></IconButton>
      </header>

      <button className="import-media-button" onClick={() => setImportOpen(true)}>
        <Import size={16} /> Import media
      </button>

      <div className="drawer-controls">
        <label className="media-search"><Search size={15} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search media" /></label>
        <IconButton label={view === "grid" ? "List view" : "Grid view"} onClick={() => setView(view === "grid" ? "list" : "grid")}>
          {view === "grid" ? <List size={16} /> : <Film size={16} />}
        </IconButton>
      </div>

      <div className={`asset-browser ${view}`} data-testid="media-grid">
        {assets.map((asset) => (
          <button key={asset.id} className="asset-card" title={asset.path} onClick={() => choose(asset, false)} onDoubleClick={() => choose(asset, true)}>
            <span className="asset-thumbnail" style={{ backgroundImage: `url("${asset.thumbnailUrl}")` }}>
              <i>{asset.kind === "video" ? <Video size={12} /> : <Music2 size={12} />}</i>
              <time>{formatDuration(asset.durationFrames, fps)}</time>
            </span>
            <span className="asset-copy"><strong>{asset.name}</strong><small>{asset.kind === "video" ? `${asset.width} × ${asset.height}` : "Audio"}</small></span>
          </button>
        ))}
        {!assets.length && <div className="drawer-empty"><Film size={25} /><p>{query ? "No matching media" : "Import media to begin editing"}</p></div>}
      </div>
    </aside>
  );
}
