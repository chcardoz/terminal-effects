import { Captions, Expand, Maximize2, Pause, Play, Ratio, SkipBack, SkipForward, StepBack, StepForward, Volume2, VolumeX } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { clamp, formatTime } from "../lib/time";
import { useEditorStore } from "../store/editor";
import { IconButton } from "./IconButton";

export function PreviewMonitor() {
  const data = useEditorStore((state) => state.data);
  const playing = useEditorStore((state) => state.playing);
  const muted = useEditorStore((state) => state.muted);
  const setPlaying = useEditorStore((state) => state.setPlaying);
  const setMuted = useEditorStore((state) => state.setMuted);
  const setFrame = useEditorStore((state) => state.setFrame);
  const persistFrame = useEditorStore((state) => state.persistFrame);
  const setStatus = useEditorStore((state) => state.setStatus);
  const videoRef = useRef<HTMLVideoElement>(null);
  const frameRef = useRef<HTMLImageElement>(null);
  const animationRef = useRef(0);
  const startTimeRef = useRef(0);
  const startFrameRef = useRef(0);
  const [fallback, setFallback] = useState(false);
  const [loading, setLoading] = useState(false);

  const fps = data?.fpsValue ?? 30;
  const frame = data?.project.playheadFrame ?? 0;
  const duration = data?.durationFrames ?? 0;
  const project = data?.project;

  const activeClip = useMemo(() => {
    if (!project) return undefined;
    return [...project.clips].reverse().find((clip) => {
      const asset = project.assets.find((candidate) => candidate.id === clip.assetId);
      return asset?.kind === "video" && clip.startFrame <= frame && frame < clip.startFrame + clip.durationFrames;
    });
  }, [project, frame]);
  const activeAsset = project?.assets.find((asset) => asset.id === activeClip?.assetId);
  const sourceTime = activeClip ? (activeClip.sourceInFrame + frame - activeClip.startFrame) / fps : 0;

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !activeAsset || !activeClip) {
      if (video) { video.pause(); video.removeAttribute("src"); video.load(); }
      return;
    }
    setFallback(false);
    setLoading(true);
    video.src = activeAsset.mediaUrl;
    video.load();
    const ready = () => {
      video.currentTime = clamp(sourceTime, 0, Number.isFinite(video.duration) ? video.duration : sourceTime);
      setLoading(false);
      if (useEditorStore.getState().playing) void video.play().catch(() => undefined);
    };
    video.addEventListener("loadedmetadata", ready, { once: true });
    return () => video.removeEventListener("loadedmetadata", ready);
    // Switching clips should reload; ordinary playhead movement is handled separately.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeClip?.id, activeAsset?.mediaUrl]);

  useEffect(() => {
    const video = videoRef.current;
    if (!playing && video && activeClip && Number.isFinite(video.duration) && Math.abs(video.currentTime - sourceTime) > 0.04) {
      video.currentTime = clamp(sourceTime, 0, video.duration);
    }
    if ((!playing || fallback) && frameRef.current) {
      frameRef.current.src = `frame?frame=${frame}&revision=${project?.revision ?? 0}`;
    }
  }, [frame, playing, activeClip, sourceTime, fallback, project?.revision]);

  const stop = useCallback(() => {
    cancelAnimationFrame(animationRef.current);
    videoRef.current?.pause();
    setPlaying(false);
    persistFrame(true);
    setStatus("Paused");
  }, [persistFrame, setPlaying, setStatus]);

  const tick = useCallback((now: number) => {
    const state = useEditorStore.getState();
    if (!state.playing || !state.data) return;
    const next = startFrameRef.current + (now - startTimeRef.current) / 1000 * state.data.fpsValue;
    if (next >= state.data.durationFrames) {
      state.setFrame(state.data.durationFrames, true);
      stop();
      return;
    }
    state.setFrame(next, false);
    animationRef.current = requestAnimationFrame(tick);
  }, [stop]);

  const toggle = () => {
    if (playing) return stop();
    const state = useEditorStore.getState();
    if (!state.data || state.data.durationFrames === 0) return;
    if (state.data.project.playheadFrame >= state.data.durationFrames) state.setFrame(0, false);
    startFrameRef.current = useEditorStore.getState().data?.project.playheadFrame ?? 0;
    startTimeRef.current = performance.now();
    setPlaying(true);
    setStatus("Playing");
    void videoRef.current?.play().catch(() => undefined);
    animationRef.current = requestAnimationFrame(tick);
  };

  useEffect(() => () => cancelAnimationFrame(animationRef.current), []);

  const hasMedia = Boolean(project?.assets.length);
  return (
    <section className="preview-monitor" data-testid="viewer">
      <header className="monitor-header">
        <div><strong>Program</strong><span>{activeAsset?.name ?? (hasMedia ? "No video at playhead" : "No media")}</span></div>
        <div className="monitor-meta"><span>{project ? `${project.width} × ${project.height}` : "—"}</span><span>{fps.toFixed(fps % 1 ? 2 : 0)} fps</span></div>
      </header>

      <div className={`preview-stage ${!playing || fallback ? "is-still" : ""}`}>
        {activeAsset && <div className="floating-toolbar" aria-label="Preview tools">
          <button><Ratio size={15} /> 16:9</button>
          <button><Expand size={15} /> Fit</button>
          <IconButton label="Fullscreen"><Maximize2 size={15} /></IconButton>
        </div>}
        <video ref={videoRef} muted={muted} playsInline preload="auto" onError={() => { setFallback(true); setLoading(false); setStatus("Using FFmpeg preview for this codec"); }} />
        <img ref={frameRef} className="preview-fallback" alt="Current program frame" />
        {!activeClip && <div className="empty-stage"><Play size={29} /><strong>{hasMedia ? "Move the playhead over a clip" : "Import media to start editing"}</strong><span>The program monitor shows your final sequence.</span></div>}
        {loading && <span className="preview-loading">Loading preview…</span>}
      </div>

      <footer className="transport-bar">
        <div className="transport-secondary">
          <IconButton label={muted ? "Unmute" : "Mute"} onClick={() => setMuted(!muted)}>{muted ? <VolumeX size={17} /> : <Volume2 size={17} />}</IconButton>
          <IconButton label="Captions" disabled><Captions size={17} /></IconButton>
        </div>
        <div className="transport-controls">
          <IconButton label="Go to start" onClick={() => setFrame(0)}><SkipBack size={17} /></IconButton>
          <IconButton label="Previous frame" onClick={() => setFrame(frame - 1)}><StepBack size={17} /></IconButton>
          <button className="transport-play" data-testid="play" aria-label={playing ? "Pause" : "Play"} onClick={toggle}>{playing ? <Pause size={17} fill="currentColor" /> : <Play size={17} fill="currentColor" />}</button>
          <IconButton label="Next frame" onClick={() => setFrame(frame + 1)}><StepForward size={17} /></IconButton>
          <IconButton label="Go to end" onClick={() => setFrame(duration)}><SkipForward size={17} /></IconButton>
        </div>
        <time className="program-time"><b>{formatTime(frame, fps)}</b><span>/</span>{formatTime(duration, fps)}</time>
      </footer>
    </section>
  );
}
