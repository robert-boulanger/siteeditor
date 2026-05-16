import { useCallback, useEffect, useRef } from "react";

const MIN_WIDTH = 180;
const MAX_WIDTH = 600;
const STORAGE_KEY = "siteeditor:sidebarWidth";

export function loadSidebarWidth(): number {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return 280;
  const n = Number(raw);
  if (!Number.isFinite(n)) return 280;
  return clamp(n);
}

function clamp(n: number): number {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(n)));
}

/** Vertikaler Drag-Handle zwischen Sidebar und Main. Updated `--sidebar-width`
 *  per Mouse-Move, persistiert in localStorage erst beim Loslassen. */
export function SidebarSplitter({ onChange }: { onChange: (width: number) => void }) {
  const dragging = useRef(false);

  const onMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!dragging.current) return;
      const w = clamp(e.clientX);
      document.documentElement.style.setProperty("--sidebar-width", `${w}px`);
      onChange(w);
    },
    [onChange],
  );

  const onMouseUp = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    // Aktuellen Wert persistieren
    const w = getComputedStyle(document.documentElement).getPropertyValue("--sidebar-width").trim();
    const n = parseInt(w, 10);
    if (Number.isFinite(n)) localStorage.setItem(STORAGE_KEY, String(n));
  }, []);

  useEffect(() => {
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [onMouseMove, onMouseUp]);

  return (
    <div
      className="sidebar-splitter"
      role="separator"
      aria-orientation="vertical"
      title="Sidebar-Breite anpassen — Doppelklick = zurücksetzen"
      onMouseDown={(e) => {
        e.preventDefault();
        dragging.current = true;
        document.body.style.cursor = "col-resize";
        document.body.style.userSelect = "none";
      }}
      onDoubleClick={() => {
        const w = 280;
        document.documentElement.style.setProperty("--sidebar-width", `${w}px`);
        localStorage.setItem(STORAGE_KEY, String(w));
        onChange(w);
      }}
    />
  );
}
