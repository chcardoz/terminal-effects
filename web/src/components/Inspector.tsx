import { Clock3, FileVideo2, MousePointer2, RotateCcw, RotateCw, Scaling, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useEditorStore } from "../store/editor";
import type { FitMode } from "../types";
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
  const [rotation, setRotation] = useState<0 | 90 | 180 | 270>(0);
  const [fit, setFit] = useState<FitMode>("contain");
  const [positionX, setPositionX] = useState(0.5);
  const [positionY, setPositionY] = useState(0.5);

  useEffect(() => {
    if (!clip) return;
    setStart(clip.startFrame);
    setSourceIn(clip.sourceInFrame);
    setDuration(clip.durationFrames);
    setRotation(clip.transform.rotationDegrees);
    setFit(clip.transform.fit);
    setPositionX(clip.transform.positionX);
    setPositionY(clip.transform.positionY);
  }, [clip]);

  const applyTransform = (reset = false) => {
    if (!clip) return;
    void runAction("api/transform", reset ? { clipId: clip.id, reset: true } : {
      clipId: clip.id,
      rotationDegrees: rotation,
      fit,
      positionX,
      positionY,
      reset: false,
    }, reset ? "Transform reset" : "Transform updated");
  };

  const rotate = (amount: -90 | 90) => {
    const next = ((rotation + amount + 360) % 360) as 0 | 90 | 180 | 270;
    setRotation(next);
  };

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

          {asset.kind === "video" && <section className="property-group transform-group">
            <h3><Scaling size={14} /> Transform</h3>
            <div className="rotation-controls">
              <button type="button" title="Rotate left" onClick={() => rotate(-90)}><RotateCcw size={15} /></button>
              <strong>{rotation}°</strong>
              <button type="button" title="Rotate right" onClick={() => rotate(90)}><RotateCw size={15} /></button>
            </div>
            <div className="fit-controls" aria-label="Frame fitting">
              <button type="button" className={fit === "contain" ? "is-active" : ""} onClick={() => setFit("contain")}>Contain</button>
              <button type="button" className={fit === "cover" ? "is-active" : ""} onClick={() => setFit("cover")}>Cover</button>
            </div>
            <TransformSlider label="Horizontal focus" value={positionX} onChange={setPositionX} />
            <TransformSlider label="Vertical focus" value={positionY} onChange={setPositionY} />
            <div className="transform-actions">
              <button type="button" onClick={() => applyTransform(true)}>Reset</button>
              <button type="button" onClick={() => applyTransform(false)}>Apply transform</button>
            </div>
          </section>}

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

function TransformSlider({ label, value, onChange }: { label: string; value: number; onChange(value: number): void }) {
  return <label className="transform-slider"><span>{label}</span><div><input type="range" min={0} max={1} step={0.01} value={value} onChange={(event) => onChange(Number(event.target.value))} /><output>{value.toFixed(2)}</output></div></label>;
}

function PropertyNumber({ label, value, min, onChange }: { label: string; value: number; min: number; onChange(value: number): void }) {
  return <label className="property-row"><span>{label}</span><span className="frame-input"><input type="number" value={value} min={min} step={1} onChange={(event) => onChange(Number(event.target.value))} /><small>fr</small></span></label>;
}
