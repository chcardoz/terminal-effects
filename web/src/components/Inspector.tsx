import { Clock3, FileVideo2, MousePointer2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useEditorStore } from "../store/editor";
import { IconButton } from "./IconButton";

export function Inspector() {
  const data = useEditorStore((state) => state.data);
  const rightPanel = useEditorStore((state) => state.rightPanel);
  const setRightPanel = useEditorStore((state) => state.setRightPanel);
  const runAction = useEditorStore((state) => state.runAction);
  const clip = data?.project.clips.find((candidate) => candidate.id === data.project.selectedClipId);
  const asset = data?.project.assets.find((candidate) => candidate.id === clip?.assetId);
  const [start, setStart] = useState(0);
  const [sourceIn, setSourceIn] = useState(0);
  const [duration, setDuration] = useState(1);

  useEffect(() => {
    if (!clip) return;
    setStart(clip.startFrame);
    setSourceIn(clip.sourceInFrame);
    setDuration(clip.durationFrames);
  }, [clip]);

  if (!rightPanel) return null;
  return (
    <aside className="properties-panel">
      <header className="properties-header"><div><strong>Properties</strong><span>{asset?.name ?? "No selection"}</span></div><IconButton label="Close properties" onClick={() => setRightPanel(null)}><X size={16} /></IconButton></header>
      {!clip || !asset ? (
        <div className="properties-empty"><MousePointer2 size={25} /><strong>Select a timeline clip</strong><span>Timing and source information will appear here.</span></div>
      ) : (
        <form className="properties-form" onSubmit={(event) => { event.preventDefault(); void runAction("api/trim", { clipId: clip.id, startFrame: start, sourceInFrame: sourceIn, durationFrames: duration }, "Clip timing updated"); }}>
          <div className="selected-source">
            <span className="selected-source-thumb" style={{ backgroundImage: `url("${asset.thumbnailUrl}")` }} />
            <div><small>{asset.kind}</small><strong>{asset.name}</strong><span>{clip.id}</span></div>
          </div>

          <section className="property-group">
            <h3><Clock3 size={14} /> Timing</h3>
            <PropertyNumber label="Timeline start" value={start} min={0} onChange={setStart} />
            <PropertyNumber label="Source in" value={sourceIn} min={0} onChange={setSourceIn} />
            <PropertyNumber label="Duration" value={duration} min={1} onChange={setDuration} />
          </section>

          <section className="property-group source-information">
            <h3><FileVideo2 size={14} /> Source</h3>
            <dl><div><dt>Resolution</dt><dd>{asset.width ? `${asset.width} × ${asset.height}` : "Audio"}</dd></div><div><dt>Audio</dt><dd>{asset.hasAudio ? "Embedded" : "None"}</dd></div><div><dt>Length</dt><dd>{asset.durationFrames} fr</dd></div></dl>
          </section>
          <button type="submit" className="apply-button">Apply timing</button>
        </form>
      )}
    </aside>
  );
}

function PropertyNumber({ label, value, min, onChange }: { label: string; value: number; min: number; onChange(value: number): void }) {
  return <label className="property-row"><span>{label}</span><span className="frame-input"><input type="number" value={value} min={min} step={1} onChange={(event) => onChange(Number(event.target.value))} /><small>fr</small></span></label>;
}
