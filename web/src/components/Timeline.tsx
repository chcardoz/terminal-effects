import { Eye, Lock, Magnet, MousePointer2, Scissors, Trash2, Volume2, ZoomIn, ZoomOut } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { clamp, formatRuler, formatTime } from "../lib/time";
import { useEditorStore } from "../store/editor";
import type { Clip, TrackKind } from "../types";
import { IconButton } from "./IconButton";

const GUTTER = 104;

export function Timeline() {
  const data = useEditorStore((state) => state.data);
  const zoom = useEditorStore((state) => state.zoom);
  const editTool = useEditorStore((state) => state.editTool);
  const snapping = useEditorStore((state) => state.snapping);
  const setZoom = useEditorStore((state) => state.setZoom);
  const setEditTool = useEditorStore((state) => state.setEditTool);
  const setSnapping = useEditorStore((state) => state.setSnapping);
  const setFrame = useEditorStore((state) => state.setFrame);
  const selectClip = useEditorStore((state) => state.selectClip);
  const runAction = useEditorStore((state) => state.runAction);
  const status = useEditorStore((state) => state.status);
  const scroller = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState(800);

  useEffect(() => {
    if (!scroller.current) return;
    const observer = new ResizeObserver(([entry]) => setViewport(entry.contentRect.width));
    observer.observe(scroller.current);
    return () => observer.disconnect();
  }, []);

  if (!data) return <section className="timeline-shell" />;
  const { project, durationFrames, fpsValue: fps } = data;
  const width = Math.max(viewport, GUTTER + durationFrames / fps * zoom + 180);
  const selected = project.clips.find((clip) => clip.id === project.selectedClipId);

  const videoClips = project.clips.filter((clip) => project.assets.find((asset) => asset.id === clip.assetId)?.kind === "video");
  const explicitAudio = project.clips.filter((clip) => project.assets.find((asset) => asset.id === clip.assetId)?.kind === "audio");
  const linkedAudio = videoClips.filter((clip) => project.assets.find((asset) => asset.id === clip.assetId)?.hasAudio);

  const split = () => selected && void runAction("api/split", { clipId: selected.id, frame: project.playheadFrame }, "Clip split");
  const remove = () => selected && void runAction("api/remove", { clipId: selected.id }, "Clip removed");
  const seekFromPointer = (event: React.PointerEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest(".timeline-clip, .track-header")) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const local = event.clientX - rect.left + (scroller.current?.scrollLeft ?? 0) - GUTTER;
    setFrame(local / zoom * fps);
  };

  return (
    <section className="timeline-shell" data-testid="timeline">
      <header className="timeline-toolbar">
        <div className="sequence-label"><strong>{project.name}</strong><span>{formatTime(durationFrames, fps)}</span></div>
        <div className="timeline-edit-tools">
          <IconButton label="Selection tool (V)" active={editTool === "select"} onClick={() => setEditTool("select")}><MousePointer2 size={16} /></IconButton>
          <IconButton label="Blade tool (B)" active={editTool === "blade"} onClick={() => setEditTool("blade")}><Scissors size={16} /></IconButton>
          <i />
          <IconButton label="Split selected clip (S)" disabled={!selected} onClick={split}><Scissors size={16} /></IconButton>
          <IconButton label="Delete selected clip" disabled={!selected} onClick={remove}><Trash2 size={16} /></IconButton>
          <IconButton label="Snapping" active={snapping} onClick={() => setSnapping(!snapping)}><Magnet size={16} /></IconButton>
        </div>
        <div className="timeline-zoom"><ZoomOut size={14} /><input aria-label="Timeline zoom" type="range" min={18} max={190} value={zoom} onChange={(event) => setZoom(Number(event.target.value))} /><ZoomIn size={14} /></div>
      </header>

      <div ref={scroller} className="timeline-scroll" onPointerDown={seekFromPointer}>
        <div className="timeline-canvas" style={{ width }}>
          <Ruler durationFrames={durationFrames} fps={fps} zoom={zoom} width={width} />
          <TrackRow id="V1" name="Video 1" kind="video" clips={videoClips} zoom={zoom} fps={fps} />
          <TrackRow id="A1" name="Embedded audio" kind="audio" clips={[...explicitAudio, ...linkedAudio]} zoom={zoom} fps={fps} linkedIds={new Set(linkedAudio.map((clip) => clip.id))} />
          <div className="timeline-playhead" style={{ left: GUTTER + project.playheadFrame / fps * zoom }}><span /><time>{formatTime(project.playheadFrame, fps).slice(3)}</time></div>
        </div>
      </div>
      <footer className="timeline-status"><span>{status}</span><span>Chromium renderer</span><span>Revision {project.revision}</span></footer>
    </section>
  );
}

function Ruler({ durationFrames, fps, zoom, width }: { durationFrames: number; fps: number; zoom: number; width: number }) {
  const seconds = Math.max(durationFrames / fps + 3, (width - GUTTER) / zoom);
  const major = zoom >= 135 ? 1 : zoom >= 68 ? 2 : zoom >= 34 ? 5 : 10;
  const minor = major / 4;
  const marks = [];
  for (let time = 0; time <= seconds; time += minor) {
    const isMajor = Math.abs(time / major - Math.round(time / major)) < 0.001;
    marks.push(<span key={time} className={isMajor ? "ruler-mark" : "ruler-mark minor"} style={{ left: GUTTER + time * zoom }}>{isMajor ? formatRuler(time * fps, fps) : ""}</span>);
  }
  return <div className="timeline-ruler"><div className="ruler-corner">TIME</div>{marks}</div>;
}

function TrackRow({ id, name, kind, clips, zoom, fps, linkedIds = new Set() }: { id: string; name: string; kind: TrackKind; clips: Clip[]; zoom: number; fps: number; linkedIds?: Set<string> }) {
  return (
    <div className={`timeline-track ${kind}`}>
      <header className="track-header"><div><strong>{id}</strong><span>{name}</span></div><div className="track-switches">{kind === "video" ? <Eye size={13} /> : <Volume2 size={13} />}<Lock size={13} /></div></header>
      <div className="track-clip-area">
        {clips.map((clip) => <TimelineClip key={`${kind}-${clip.id}`} clip={clip} kind={kind} zoom={zoom} fps={fps} linked={linkedIds.has(clip.id)} />)}
      </div>
    </div>
  );
}

function TimelineClip({ clip, kind, zoom, fps, linked }: { clip: Clip; kind: TrackKind; zoom: number; fps: number; linked: boolean }) {
  const data = useEditorStore((state) => state.data)!;
  const selected = data.project.selectedClipId === clip.id;
  const editTool = useEditorStore((state) => state.editTool);
  const snapping = useEditorStore((state) => state.snapping);
  const selectClip = useEditorStore((state) => state.selectClip);
  const runAction = useEditorStore((state) => state.runAction);
  const setInteracting = useEditorStore((state) => state.setInteracting);
  const setStatus = useEditorStore((state) => state.setStatus);
  const asset = data.project.assets.find((candidate) => candidate.id === clip.assetId)!;
  const [draft, setDraft] = useState<{ start: number; source: number; duration: number } | null>(null);
  const timing = draft ?? { start: clip.startFrame, source: clip.sourceInFrame, duration: clip.durationFrames };

  const snapFrame = (frame: number) => {
    if (!snapping) return frame;
    const threshold = Math.max(1, Math.round(7 / zoom * fps));
    const targets = [0, data.project.playheadFrame, ...data.project.clips.filter((candidate) => candidate.id !== clip.id).flatMap((candidate) => [candidate.startFrame, candidate.startFrame + candidate.durationFrames])];
    return targets.reduce((best, target) => Math.abs(target - frame) <= threshold && Math.abs(target - frame) < Math.abs(best - frame) ? target : best, frame);
  };

  const begin = (event: React.PointerEvent, mode: "move" | "left" | "right") => {
    if (event.button !== 0 || (linked && mode !== "move")) return;
    event.preventDefault();
    event.stopPropagation();
    selectClip(clip.id);
    if (editTool === "blade" && mode === "move") {
      const area = (event.currentTarget as HTMLElement).closest(".track-clip-area")!.getBoundingClientRect();
      const frame = Math.round((event.clientX - area.left) / zoom * fps);
      void runAction("api/split", { clipId: clip.id, frame }, "Clip split");
      return;
    }
    if (linked) return;
    const originX = event.clientX;
    let latest = { start: clip.startFrame, source: clip.sourceInFrame, duration: clip.durationFrames };
    let moved = false;
    setInteracting(true);
    const move = (pointer: PointerEvent) => {
      const delta = Math.round((pointer.clientX - originX) / zoom * fps);
      moved ||= Math.abs(pointer.clientX - originX) >= 2;
      if (mode === "move") latest = { ...latest, start: snapFrame(Math.max(0, clip.startFrame + delta)) };
      if (mode === "left") {
        const limited = clamp(delta, -clip.sourceInFrame, clip.durationFrames - 1);
        latest = { start: Math.max(0, clip.startFrame + limited), source: clip.sourceInFrame + limited, duration: clip.durationFrames - limited };
      }
      if (mode === "right") latest = { start: clip.startFrame, source: clip.sourceInFrame, duration: clamp(clip.durationFrames + delta, 1, asset.durationFrames - clip.sourceInFrame) };
      setDraft(latest);
      setStatus(mode === "move" ? `Moving to ${formatTime(latest.start, fps)}` : `Duration ${formatTime(latest.duration, fps)}`);
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      setInteracting(false);
      if (!moved) { setDraft(null); return; }
      if (mode === "move") void runAction("api/move", { clipId: clip.id, trackId: clip.trackId, frame: latest.start }, "Clip moved");
      else void runAction("api/trim", { clipId: clip.id, startFrame: latest.start, sourceInFrame: latest.source, durationFrames: latest.duration }, "Clip trimmed");
      setDraft(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish, { once: true });
  };

  const background = kind === "video" ? clip.filmstripUrl : clip.waveformUrl;
  return (
    <div className={`timeline-clip ${kind} ${selected ? "is-selected" : ""} ${linked ? "is-linked" : ""} ${draft ? "is-dragging" : ""}`} data-clip-id={clip.id} style={{ left: timing.start / fps * zoom, width: Math.max(9, timing.duration / fps * zoom), backgroundImage: `url("${background}")` }} onPointerDown={(event) => begin(event, "move")}>
      {!linked && <button className="trim-handle left" aria-label="Trim clip start" onPointerDown={(event) => begin(event, "left")} />}
      <span className="clip-title">{kind === "audio" ? <Volume2 size={11} /> : null}<strong>{asset.name}</strong></span>
      {!linked && <button className="trim-handle right" aria-label="Trim clip end" onPointerDown={(event) => begin(event, "right")} />}
    </div>
  );
}
