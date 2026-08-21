import { Box, Text } from "@terminal-effects/pixel-react";
import type { EngineInfo, PointerEvent, Surface, WheelEvent } from "@terminal-effects/pixel-react";

export interface AppLayout {
  width: number;
  height: number;
  rem: number;
  page: { x: number; y: number; width: number; height: number };
}

export interface PopupView {
  title: string;
  host: string;
  loading: boolean;
  width: number;
  height: number;
}

export function AppView({
  layout,
  colors,
  font,
  pageSurface,
  popupSurface,
  popup,
  onPagePointer,
  onPageWheel,
  onPopupPointer,
  onPopupWheel,
  onPopupClose,
}: {
  layout: AppLayout;
  colors: EngineInfo["colors"];
  font: number;
  pageSurface: Surface;
  popupSurface: Surface;
  popup: PopupView | null;
  onPagePointer(event: PointerEvent): void;
  onPageWheel(event: WheelEvent): void;
  onPopupPointer(event: PointerEvent): void;
  onPopupWheel(event: WheelEvent): void;
  onPopupClose(): void;
}) {
  const background = colors.background ?? ([30, 32, 38, 255] as const);
  const foreground = colors.foreground ?? ([230, 232, 238, 255] as const);
  const muted = colors.palette[8] ?? ([142, 146, 158, 255] as const);
  const panel = colors.palette[0] ?? ([38, 41, 49, 255] as const);
  const border = colors.palette[8] ?? ([82, 87, 101, 255] as const);
  const rem = layout.rem;
  const headerHeight = Math.round(rem * 1.7);
  const popupTop = popup
    ? layout.page.y + Math.max(Math.round(rem * 0.5), Math.round((layout.page.height - popup.height - headerHeight) / 2))
    : 0;
  const popupLeft = popup
    ? layout.page.x + Math.round((layout.page.width - popup.width) / 2)
    : 0;

  return (
    <Box
      style={{
        width: layout.width,
        height: layout.height,
        background,
        color: foreground,
        fontSize: rem,
        font,
      }}
    >
      <Box
        surface={pageSurface}
        style={{
          position: "absolute",
          inset: { top: layout.page.y, left: layout.page.x },
          width: layout.page.width,
          height: layout.page.height,
          background,
        }}
        onPointer={onPagePointer}
        onWheel={onPageWheel}
      />
      {popup && (
        <>
          <Box
            style={{
              position: "absolute",
              inset: { top: layout.page.y, left: layout.page.x },
              width: layout.page.width,
              height: layout.page.height,
              background: [8, 9, 12, 150],
            }}
            onPointer={(event) => {
              if (event.kind === "down") onPopupClose();
            }}
            onWheel={() => {}}
          />
          <Box
            style={{
              position: "absolute",
              inset: { top: popupTop, left: popupLeft },
              width: popup.width,
              flexDirection: "column",
              background,
              cornerRadius: rem * 0.5,
              border: { width: 1, color: border },
              overflow: "hidden",
            }}
            onPointer={() => {}}
            onWheel={() => {}}
          >
            <Box
              style={{
                height: headerHeight,
                alignItems: "center",
                gap: rem * 0.5,
                padding: { left: rem * 0.65, right: rem * 0.5 },
                background: panel,
                border: { bottom: [1, border] },
              }}
            >
              <Text style={{ fontSize: rem * 0.78, wrap: false, selectable: false }}>
                {popup.title || (popup.loading ? "loading…" : popup.host)}
              </Text>
              <Box style={{ flexGrow: 1, flexBasis: 0 }} />
              <Text style={{ fontSize: rem * 0.72, color: muted, wrap: false, selectable: false }}>
                {popup.host}
              </Text>
              <Box
                style={{
                  width: rem * 1.15,
                  height: rem * 1.15,
                  alignItems: "center",
                  justifyContent: "center",
                  cornerRadius: rem * 0.3,
                  hoverBackground: border,
                }}
                onClick={onPopupClose}
              >
                <Text style={{ fontSize: rem, color: muted, selectable: false }}>×</Text>
              </Box>
            </Box>
            <Box
              surface={popupSurface}
              style={{ width: popup.width, height: popup.height, background }}
              onPointer={onPopupPointer}
              onWheel={onPopupWheel}
            />
          </Box>
        </>
      )}
    </Box>
  );
}
