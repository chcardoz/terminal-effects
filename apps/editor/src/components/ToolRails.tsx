import { Captions, Files, Music2, Palette, SlidersHorizontal, Sparkles, Type, Volume2 } from "lucide-react";
import type { LeftPanel, RightPanel } from "../types";
import { useEditorStore } from "../store/editor";

const leftTools: Array<{ id: LeftPanel; label: string; icon: typeof Files; disabled?: boolean }> = [
  { id: "media", label: "Your media", icon: Files },
  { id: "audio", label: "Audio", icon: Music2 },
  { id: "text", label: "Text", icon: Type, disabled: true },
  { id: "transitions", label: "Transitions", icon: Sparkles, disabled: true },
];

const rightTools: Array<{ id: Exclude<RightPanel, null>; label: string; icon: typeof Files; disabled?: boolean }> = [
  { id: "properties", label: "Properties", icon: SlidersHorizontal },
  { id: "audio", label: "Audio controls", icon: Volume2, disabled: true },
  { id: "color", label: "Color", icon: Palette, disabled: true },
];

export function LeftToolRail() {
  const active = useEditorStore((state) => state.leftPanel);
  const setPanel = useEditorStore((state) => state.setLeftPanel);
  return (
    <nav className="tool-rail left-tool-rail" aria-label="Content tools">
      {leftTools.map(({ id, label, icon: Icon, disabled }) => (
        <button key={id} className={active === id ? "is-active" : ""} disabled={disabled} title={disabled ? `${label} — coming later` : label} onClick={() => setPanel(id)}>
          <Icon size={19} strokeWidth={1.7} />
          <span>{label.split(" ")[0]}</span>
        </button>
      ))}
      <div className="rail-spacer" />
      <button disabled title="Captions — coming later"><Captions size={19} /><span>Captions</span></button>
    </nav>
  );
}

export function RightToolRail() {
  const active = useEditorStore((state) => state.rightPanel);
  const setPanel = useEditorStore((state) => state.setRightPanel);
  return (
    <nav className="tool-rail right-tool-rail" aria-label="Clip tools">
      {rightTools.map(({ id, label, icon: Icon, disabled }) => (
        <button key={id} className={active === id ? "is-active" : ""} disabled={disabled} title={disabled ? `${label} — coming later` : label} onClick={() => setPanel(active === id ? null : id)}>
          <Icon size={19} strokeWidth={1.7} />
        </button>
      ))}
    </nav>
  );
}
