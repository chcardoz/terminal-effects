"use strict";

const $ = (selector) => document.querySelector(selector);
const state = {
  data: null,
  zoom: 76,
  playing: false,
  muted: false,
  tool: "select",
  snap: true,
  raf: 0,
  playStartedAt: 0,
  playStartedFrame: 0,
  previewClipId: null,
  persistTimer: 0,
  toastTimer: 0,
  dragging: null,
};

async function api(path, body) {
  const options = body === undefined
    ? { cache: "no-store" }
    : { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) };
  const response = await fetch(path, options);
  const payload = await response.json().catch(() => ({ error: response.statusText }));
  if (!response.ok) throw new Error(payload.error || `Request failed (${response.status})`);
  return payload;
}

function project() { return state.data?.project; }
function fps() { return state.data?.fpsValue || 30; }
function durationFrames() { return Math.max(0, state.data?.durationFrames || 0); }
function assetFor(id) { return project()?.assets.find((asset) => asset.id === id); }
function selectedClip() { return project()?.clips.find((clip) => clip.id === project().selectedClipId); }
function clamp(value, min, max) { return Math.min(max, Math.max(min, value)); }

function formatTime(frame) {
  const totalMs = Math.round(Math.max(0, frame) / fps() * 1000);
  const hours = Math.floor(totalMs / 3600000);
  const minutes = Math.floor(totalMs / 60000) % 60;
  const seconds = Math.floor(totalMs / 1000) % 60;
  const millis = totalMs % 1000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

function shortTime(frame) {
  const seconds = Math.max(0, frame) / fps();
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
}

function showToast(message, error = false) {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", error);
  toast.classList.add("visible");
  clearTimeout(state.toastTimer);
  state.toastTimer = setTimeout(() => toast.classList.remove("visible"), 2800);
}

function setStatus(message) { $("#statusMessage").textContent = message; }
function setSaving(saving) {
  $("#saveState").lastChild.textContent = saving ? " Saving…" : " Saved";
  $("#saveState").classList.toggle("saving", saving);
}

async function loadProject({ silent = false, force = false } = {}) {
  try {
    const data = await api("api/project");
    const changed = force || !state.data || data.project.revision !== state.data.project.revision;
    const remoteStateChanged = !state.data || data.project.playheadFrame !== state.data.project.playheadFrame || data.project.selectedClipId !== state.data.project.selectedClipId;
    state.data = data;
    if (changed || remoteStateChanged) renderAll();
    $("#app").setAttribute("aria-busy", "false");
    if (!silent) setStatus("Ready");
  } catch (error) {
    setStatus(error.message);
    showToast(error.message, true);
  }
}

function renderAll() {
  const p = project();
  if (!p) return;
  $("#projectPath").textContent = state.data.root;
  $("#sequenceName").textContent = p.name;
  $("#assetCount").textContent = p.assets.length;
  $("#resolutionLabel").textContent = `${p.width} × ${p.height}`;
  $("#fpsLabel").textContent = `${fps().toFixed(fps() % 1 ? 2 : 0)} fps`;
  $("#durationTimecode").textContent = formatTime(durationFrames());
  $("#revisionLabel").textContent = `Revision ${p.revision}`;
  renderMedia();
  renderTimeline();
  renderInspector();
  updatePlayheadVisual();
  syncPreview(true);
}

function renderMedia() {
  const grid = $("#mediaGrid");
  const query = $("#mediaSearch").value.trim().toLowerCase();
  const assets = project().assets.filter((asset) => asset.name.toLowerCase().includes(query));
  grid.replaceChildren();
  if (!assets.length) {
    const empty = document.createElement("div");
    empty.className = "empty-bin";
    empty.textContent = project().assets.length ? "No matching media" : "No media yet\nUse Import to add clips";
    grid.append(empty);
    return;
  }
  const selected = selectedClip();
  for (const asset of assets) {
    const item = document.createElement("button");
    item.className = `media-item${selected?.assetId === asset.id ? " selected" : ""}`;
    item.dataset.assetId = asset.id;
    item.title = asset.path;
    const thumb = document.createElement("div");
    thumb.className = "media-thumb";
    thumb.style.backgroundImage = `url("${asset.thumbnailUrl}")`;
    thumb.innerHTML = `<span class="media-type">${asset.kind.toUpperCase()}</span><span class="media-duration">${shortTime(asset.durationFrames)}</span>`;
    const name = document.createElement("span");
    name.className = "media-name";
    name.textContent = asset.name;
    const details = document.createElement("span");
    details.className = "media-details";
    details.textContent = asset.kind === "video" ? `${asset.width}×${asset.height}` : "Audio";
    item.append(thumb, name, details);
    item.addEventListener("dblclick", () => seekToAsset(asset.id));
    item.addEventListener("click", () => selectAsset(asset.id));
    grid.append(item);
  }
}

function selectAsset(assetId) {
  const clip = project().clips.find((candidate) => candidate.assetId === assetId);
  if (!clip) return;
  selectClip(clip.id, false);
}

function seekToAsset(assetId) {
  const clip = project().clips.find((candidate) => candidate.assetId === assetId);
  if (!clip) return;
  selectClip(clip.id, false);
  setFrame(clip.startFrame, { persist: true, preview: true });
}

function timelineWidth() {
  const viewport = $("#timelineScroller").clientWidth || 900;
  return Math.max(viewport, 82 + durationFrames() / fps() * state.zoom + 180);
}

function renderRuler(width) {
  const ruler = $("#timelineRuler");
  ruler.replaceChildren();
  const seconds = Math.max(durationFrames() / fps() + 3, (width - 82) / state.zoom);
  const major = state.zoom >= 130 ? 1 : state.zoom >= 65 ? 2 : state.zoom >= 35 ? 5 : 10;
  const minor = major / 4;
  for (let time = 0; time <= seconds; time += minor) {
    const mark = document.createElement("span");
    const isMajor = Math.abs(time / major - Math.round(time / major)) < 0.001;
    mark.className = `ruler-mark${isMajor ? "" : " minor"}`;
    mark.style.left = `${time * state.zoom}px`;
    if (isMajor) mark.textContent = formatTime(Math.round(time * fps())).slice(3, 8);
    ruler.append(mark);
  }
}

function renderTimeline() {
  const width = timelineWidth();
  const content = $("#timelineContent");
  content.style.width = `${width}px`;
  renderRuler(width);
  const tracks = $("#timelineTracks");
  tracks.replaceChildren();
  for (const track of project().tracks) {
    const row = document.createElement("div");
    row.className = "timeline-track";
    row.dataset.trackId = track.id;
    row.innerHTML = `<div class="track-label"><div><strong>${track.id}</strong><span>${track.name}</span></div><div class="track-controls"><span>◉</span><span>${track.kind === "video" ? "◇" : "M"}</span></div></div>`;
    row.addEventListener("pointerdown", timelinePointerDown);
    for (const clip of project().clips.filter((candidate) => candidate.trackId === track.id)) {
      row.append(createClipElement(clip, track.kind));
    }
    tracks.append(row);
  }
}

function createClipElement(clip, trackKind) {
  const asset = assetFor(clip.assetId);
  const element = document.createElement("div");
  element.className = `clip ${trackKind}${clip.id === project().selectedClipId ? " selected" : ""}`;
  element.dataset.clipId = clip.id;
  element.style.left = `${clip.startFrame / fps() * state.zoom}px`;
  element.style.width = `${Math.max(8, clip.durationFrames / fps() * state.zoom)}px`;
  if (trackKind === "video") element.style.backgroundImage = `url("${asset.thumbnailUrl}")`;
  const label = document.createElement("div");
  label.className = "clip-label";
  label.innerHTML = `<strong>${asset?.name || "Missing media"}</strong>`;
  const left = document.createElement("span");
  left.className = "trim-handle left";
  const right = document.createElement("span");
  right.className = "trim-handle right";
  left.addEventListener("pointerdown", (event) => beginClipDrag(event, clip, "left"));
  right.addEventListener("pointerdown", (event) => beginClipDrag(event, clip, "right"));
  element.addEventListener("pointerdown", (event) => {
    if (event.target.classList.contains("trim-handle")) return;
    if (state.tool === "blade") {
      event.stopPropagation();
      const rect = event.currentTarget.parentElement.getBoundingClientRect();
      const frame = Math.round((event.clientX - rect.left) / state.zoom * fps());
      splitClip(clip.id, frame);
      return;
    }
    beginClipDrag(event, clip, "move");
  });
  element.append(label, left, right);
  return element;
}

function timelinePointerDown(event) {
  if (event.button !== 0 || event.target.closest(".clip") || event.target.closest(".track-label")) return;
  const rect = event.currentTarget.getBoundingClientRect();
  const frame = Math.round((event.clientX - rect.left) / state.zoom * fps());
  setFrame(frame, { persist: true, preview: true });
}

function selectClip(clipId, rerender = true) {
  if (!project() || project().selectedClipId === clipId) return;
  project().selectedClipId = clipId;
  document.querySelectorAll(".clip.selected").forEach((clip) => clip.classList.remove("selected"));
  document.querySelector(`.clip[data-clip-id="${CSS.escape(clipId)}"]`)?.classList.add("selected");
  renderInspector();
  renderMedia();
  api("api/select", { clipId }).catch((error) => showToast(error.message, true));
  if (rerender) syncPreview(false);
}

function snapFrame(frame, movingClipId) {
  if (!state.snap) return frame;
  const threshold = Math.max(1, Math.round(7 / state.zoom * fps()));
  const targets = [0, project().playheadFrame];
  for (const clip of project().clips) {
    if (clip.id === movingClipId) continue;
    targets.push(clip.startFrame, clip.startFrame + clip.durationFrames);
  }
  let result = frame;
  let distance = threshold + 1;
  for (const target of targets) {
    const candidate = Math.abs(target - frame);
    if (candidate < distance && candidate <= threshold) {
      result = target;
      distance = candidate;
    }
  }
  return result;
}

function beginClipDrag(event, clip, mode) {
  if (event.button !== 0) return;
  event.preventDefault();
  event.stopPropagation();
  selectClip(clip.id, false);
  const element = event.currentTarget.closest(".clip") || event.currentTarget;
  element.classList.add("dragging");
  state.dragging = {
    mode,
    clip,
    element,
    startX: event.clientX,
    startFrame: clip.startFrame,
    sourceIn: clip.sourceInFrame,
    duration: clip.durationFrames,
    nextStart: clip.startFrame,
    nextSourceIn: clip.sourceInFrame,
    nextDuration: clip.durationFrames,
  };
  window.addEventListener("pointermove", updateClipDrag);
  window.addEventListener("pointerup", finishClipDrag, { once: true });
}

function updateClipDrag(event) {
  const drag = state.dragging;
  if (!drag) return;
  const delta = Math.round((event.clientX - drag.startX) / state.zoom * fps());
  const asset = assetFor(drag.clip.assetId);
  if (drag.mode === "move") {
    drag.nextStart = snapFrame(Math.max(0, drag.startFrame + delta), drag.clip.id);
  } else if (drag.mode === "left") {
    const limited = clamp(delta, -drag.sourceIn, drag.duration - 1);
    drag.nextStart = Math.max(0, drag.startFrame + limited);
    drag.nextSourceIn = drag.sourceIn + limited;
    drag.nextDuration = drag.duration - limited;
  } else {
    const maxDuration = Math.max(1, asset.durationFrames - drag.sourceIn);
    drag.nextDuration = clamp(drag.duration + delta, 1, maxDuration);
  }
  drag.element.style.left = `${drag.nextStart / fps() * state.zoom}px`;
  drag.element.style.width = `${Math.max(8, drag.nextDuration / fps() * state.zoom)}px`;
  $("#clipStart").value = drag.nextStart;
  $("#clipSourceIn").value = drag.nextSourceIn;
  $("#clipDuration").value = drag.nextDuration;
  setStatus(drag.mode === "move" ? `Moving to ${formatTime(drag.nextStart)}` : `Trimming to ${formatTime(drag.nextDuration)}`);
}

async function finishClipDrag() {
  window.removeEventListener("pointermove", updateClipDrag);
  const drag = state.dragging;
  state.dragging = null;
  if (!drag) return;
  drag.element.classList.remove("dragging");
  try {
    setSaving(true);
    if (drag.mode === "move") {
      await api("api/move", { clipId: drag.clip.id, trackId: drag.clip.trackId, frame: drag.nextStart });
    } else {
      await api("api/trim", { clipId: drag.clip.id, startFrame: drag.nextStart, sourceInFrame: drag.nextSourceIn, durationFrames: drag.nextDuration });
    }
    await loadProject({ silent: true, force: true });
    setStatus(drag.mode === "move" ? "Clip moved" : "Clip trimmed");
  } catch (error) {
    showToast(error.message, true);
    await loadProject({ silent: true, force: true });
  } finally {
    setSaving(false);
  }
}

function renderInspector() {
  const clip = selectedClip();
  $("#inspectorEmpty").hidden = Boolean(clip);
  $("#inspectorForm").hidden = !clip;
  $("#deleteButton").disabled = !clip;
  $("#splitButton").disabled = !clip;
  if (!clip) return;
  const asset = assetFor(clip.assetId);
  $("#selectedClipKind").textContent = asset.kind.toUpperCase();
  $("#selectedClipName").textContent = asset.name;
  $("#selectedClipId").textContent = clip.id;
  $("#clipStart").value = clip.startFrame;
  $("#clipSourceIn").value = clip.sourceInFrame;
  $("#clipDuration").value = clip.durationFrames;
}

function activeVideoClip(frame) {
  return [...project().clips].reverse().find((clip) => {
    const asset = assetFor(clip.assetId);
    return asset?.kind === "video" && clip.startFrame <= frame && frame < clip.startFrame + clip.durationFrames;
  });
}

function sourceTime(clip, frame) { return (clip.sourceInFrame + frame - clip.startFrame) / fps(); }

function syncPreview(force = false) {
  if (!project()) return;
  const video = $("#previewVideo");
  const fallback = $("#previewFallback");
  const stage = $("#viewerStage");
  const clip = activeVideoClip(project().playheadFrame);
  $("#emptyViewer").hidden = Boolean(project().assets.length);
  if (!clip) {
    video.pause();
    video.removeAttribute("src");
    video.load();
    fallback.removeAttribute("src");
    stage.classList.remove("loading", "fallback");
    state.previewClipId = null;
    $("#viewerTitle").textContent = project().assets.length ? "No video at playhead" : "No media";
    return;
  }
  const asset = assetFor(clip.assetId);
  $("#emptyViewer").hidden = true;
  $("#viewerTitle").textContent = asset.name;
  const wantedTime = sourceTime(clip, project().playheadFrame);
  if (force || state.previewClipId !== clip.id) {
    state.previewClipId = clip.id;
    stage.classList.remove("fallback");
    stage.classList.add("loading");
    video.src = asset.mediaUrl;
    video.muted = state.muted;
    video.load();
    const ready = () => {
      video.currentTime = clamp(wantedTime, 0, Number.isFinite(video.duration) ? video.duration : wantedTime);
      stage.classList.remove("loading");
      if (state.playing) video.play().catch(() => {});
    };
    video.addEventListener("loadedmetadata", ready, { once: true });
  } else if (!state.playing && Number.isFinite(video.duration) && Math.abs(video.currentTime - wantedTime) > 0.04) {
    video.currentTime = clamp(wantedTime, 0, video.duration);
  }
}

function useFrameFallback() {
  const stage = $("#viewerStage");
  stage.classList.remove("loading");
  stage.classList.add("fallback");
  $("#previewFallback").src = `frame?frame=${project().playheadFrame}&revision=${project().revision}`;
  setStatus("Using FFmpeg preview for this codec");
}

function updatePlayheadVisual() {
  if (!project()) return;
  const frame = clamp(project().playheadFrame, 0, durationFrames());
  $("#timecode").textContent = formatTime(frame);
  $("#timelinePlayhead").style.left = `calc(var(--gutter) + ${frame / fps() * state.zoom}px)`;
}

function setFrame(frame, { persist = true, preview = true } = {}) {
  if (!project()) return;
  project().playheadFrame = clamp(Math.round(frame), 0, durationFrames());
  updatePlayheadVisual();
  if (preview) syncPreview(false);
  if (persist) schedulePlayheadSave();
}

function schedulePlayheadSave(immediate = false) {
  clearTimeout(state.persistTimer);
  const save = () => api("api/playhead", { frame: project().playheadFrame }).catch((error) => showToast(error.message, true));
  if (immediate) save(); else state.persistTimer = setTimeout(save, 160);
}

function togglePlayback() {
  if (!project() || durationFrames() === 0) return;
  if (state.playing) return pausePlayback();
  if (project().playheadFrame >= durationFrames()) setFrame(0, { persist: false, preview: true });
  state.playing = true;
  state.playStartedFrame = project().playheadFrame;
  state.playStartedAt = performance.now();
  $("#playButton img").src = "icons/pause.svg";
  setStatus("Playing");
  syncPreview(false);
  $("#previewVideo").play().catch(() => {});
  state.raf = requestAnimationFrame(playTick);
}

function playTick(now) {
  if (!state.playing) return;
  const frame = state.playStartedFrame + (now - state.playStartedAt) / 1000 * fps();
  if (frame >= durationFrames()) {
    setFrame(durationFrames(), { persist: true, preview: true });
    pausePlayback();
    return;
  }
  const beforeClip = state.previewClipId;
  setFrame(frame, { persist: false, preview: false });
  const active = activeVideoClip(project().playheadFrame)?.id || null;
  if (active !== beforeClip) syncPreview(false);
  if (Math.round(frame) % Math.max(1, Math.round(fps() / 4)) === 0) schedulePlayheadSave();
  state.raf = requestAnimationFrame(playTick);
}

function pausePlayback() {
  state.playing = false;
  cancelAnimationFrame(state.raf);
  $("#previewVideo").pause();
  $("#playButton img").src = "icons/play.svg";
  schedulePlayheadSave(true);
  setStatus("Paused");
}

async function action(path, body, success) {
  try {
    setSaving(true);
    await api(path, body);
    await loadProject({ silent: true, force: true });
    setStatus(success);
  } catch (error) {
    showToast(error.message, true);
    setStatus(error.message);
  } finally {
    setSaving(false);
  }
}

function splitSelected() {
  const clip = selectedClip();
  if (!clip) return showToast("Select a clip first", true);
  splitClip(clip.id, project().playheadFrame);
}

function splitClip(clipId, frame) { action("api/split", { clipId, frame }, "Clip split"); }
function deleteSelected() {
  const clip = selectedClip();
  if (clip) action("api/remove", { clipId: clip.id }, "Clip removed");
}

function setTool(tool) {
  state.tool = tool;
  $("#selectTool").classList.toggle("active", tool === "select");
  $("#bladeTool").classList.toggle("active", tool === "blade");
  setStatus(tool === "blade" ? "Blade tool — click a clip to split" : "Selection tool");
}

function openImportDialog() {
  $("#importDialog").hidden = false;
  $("#importPaths").focus();
}
function closeImportDialog() { $("#importDialog").hidden = true; }

function bindEvents() {
  $("#previewVideo").addEventListener("error", useFrameFallback);
  $("#playButton").addEventListener("click", togglePlayback);
  $("#stepBackButton").addEventListener("click", () => setFrame(project().playheadFrame - 1));
  $("#stepForwardButton").addEventListener("click", () => setFrame(project().playheadFrame + 1));
  $("#jumpStartButton").addEventListener("click", () => setFrame(0));
  $("#jumpEndButton").addEventListener("click", () => setFrame(durationFrames()));
  $("#muteButton").addEventListener("click", () => {
    state.muted = !state.muted;
    $("#previewVideo").muted = state.muted;
    $("#muteButton img").src = state.muted ? "icons/volume-x.svg" : "icons/volume-2.svg";
  });
  $("#undoButton").addEventListener("click", () => action("api/undo", {}, "Undo"));
  $("#redoButton").addEventListener("click", () => action("api/redo", {}, "Redo"));
  $("#splitButton").addEventListener("click", splitSelected);
  $("#deleteButton").addEventListener("click", deleteSelected);
  $("#selectTool").addEventListener("click", () => setTool("select"));
  $("#bladeTool").addEventListener("click", () => setTool("blade"));
  $("#snapButton").addEventListener("click", () => {
    state.snap = !state.snap;
    $("#snapButton").classList.toggle("active", state.snap);
  });
  $("#zoomSlider").addEventListener("input", (event) => {
    state.zoom = Number(event.target.value);
    renderTimeline();
    updatePlayheadVisual();
  });
  window.addEventListener("resize", () => { renderTimeline(); updatePlayheadVisual(); });
  $("#mediaSearch").addEventListener("input", renderMedia);
  $("#importButton").addEventListener("click", openImportDialog);
  $("#importIconButton").addEventListener("click", openImportDialog);
  $(".dialog-close").addEventListener("click", closeImportDialog);
  $(".dialog-cancel").addEventListener("click", closeImportDialog);
  $("#importDialog").addEventListener("pointerdown", (event) => { if (event.target === event.currentTarget) closeImportDialog(); });
  $("#importForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const paths = $("#importPaths").value.split("\n").map((path) => path.trim()).filter(Boolean);
    if (!paths.length) return;
    closeImportDialog();
    await action("api/import", { paths }, `Imported ${paths.length} file${paths.length === 1 ? "" : "s"}`);
    $("#importPaths").value = "";
  });
  $("#exportButton").addEventListener("click", async () => {
    $("#exportButton").disabled = true;
    setStatus("Exporting sequence with FFmpeg…");
    try {
      const result = await api("api/export", {});
      showToast(`Exported ${result.path}`);
      setStatus("Export complete");
    } catch (error) {
      showToast(error.message, true);
      setStatus(error.message);
    } finally {
      $("#exportButton").disabled = false;
    }
  });
  $("#inspectorForm").addEventListener("submit", (event) => {
    event.preventDefault();
    const clip = selectedClip();
    if (!clip) return;
    action("api/trim", {
      clipId: clip.id,
      startFrame: Number($("#clipStart").value),
      sourceInFrame: Number($("#clipSourceIn").value),
      durationFrames: Number($("#clipDuration").value),
    }, "Clip timing updated");
  });
  document.addEventListener("keydown", (event) => {
    const editing = event.target.matches("input, textarea");
    if (event.key === "Escape" && !$("#importDialog").hidden) return closeImportDialog();
    if (editing) return;
    const command = event.metaKey || event.ctrlKey;
    if (command && event.key.toLowerCase() === "z") {
      event.preventDefault();
      action(event.shiftKey ? "api/redo" : "api/undo", {}, event.shiftKey ? "Redo" : "Undo");
    } else if (event.code === "Space") {
      event.preventDefault(); togglePlayback();
    } else if (event.key === "ArrowLeft") {
      event.preventDefault(); setFrame(project().playheadFrame - (event.shiftKey ? Math.round(fps()) : 1));
    } else if (event.key === "ArrowRight") {
      event.preventDefault(); setFrame(project().playheadFrame + (event.shiftKey ? Math.round(fps()) : 1));
    } else if (event.key.toLowerCase() === "s") {
      splitSelected();
    } else if (event.key.toLowerCase() === "b") {
      setTool("blade");
    } else if (event.key.toLowerCase() === "v") {
      setTool("select");
    } else if (event.key === "Backspace" || event.key === "Delete") {
      event.preventDefault(); deleteSelected();
    } else if (event.key.toLowerCase() === "q" && window.terminalEffectsHost?.quit) {
      window.terminalEffectsHost.quit();
    }
  });
}

bindEvents();
loadProject({ force: true });
setInterval(() => {
  if (!state.playing && !state.dragging && document.visibilityState === "visible") loadProject({ silent: true });
}, 900);
