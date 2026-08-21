import type { WebContents } from "electron";

export function scaleZoom(contents: WebContents, ratio: number): number {
  const floor = 0.25;
  const ceiling = 5;
  const exact = contents.getZoomFactor() * ratio;
  const factor = Math.min(ceiling, Math.max(floor, Math.round(exact * 1000) / 1000));
  contents.setZoomFactor(factor);
  return factor;
}
