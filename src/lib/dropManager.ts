import { useEffect, useRef } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const IMG_EXT = ["png","jpg","jpeg","gif","webp","svg","avif"];
const VID_EXT = ["mp4","webm","mov"];

export function filterPaths(paths: string[], accept: "image" | "video" | "any"): string[] {
  if (accept === "any") return paths;
  const allowed = accept === "image" ? IMG_EXT : VID_EXT;
  return paths.filter((p) => {
    const ext = p.toLowerCase().split(".").pop() ?? "";
    return allowed.includes(ext);
  });
}

/**
 * Native File-Drops aus dem Finder/Explorer landen NICHT als HTML5-Drop im
 * Webview, sondern als Tauri-Event mit absoluten Pfaden. Wir abonnieren das
 * Event einmal global und routen es per `elementFromPoint`-Hittest an die
 * registrierten Drop-Zonen.
 */
export type AssetDropHandler = (
  paths: string[],
  position: { x: number; y: number },
) => void | Promise<void>;

type Zone = { el: HTMLElement; handler: AssetDropHandler };

const zones = new Set<Zone>();
let installed = false;
let activeEl: HTMLElement | null = null;

function pickZone(physX: number, physY: number): { zone: Zone | null; cx: number; cy: number } {
  const dpr = window.devicePixelRatio || 1;
  const cx = physX / dpr;
  const cy = physY / dpr;
  // Kleinste umschließende Zone gewinnt (innerstes Element)
  let best: { zone: Zone; area: number } | null = null;
  for (const z of zones) {
    const r = z.el.getBoundingClientRect();
    if (cx >= r.left && cx <= r.right && cy >= r.top && cy <= r.bottom) {
      const area = r.width * r.height;
      if (!best || area < best.area) best = { zone: z, area };
    }
  }
  return { zone: best?.zone ?? null, cx, cy };
}

function setActive(el: HTMLElement | null) {
  if (activeEl === el) return;
  activeEl?.classList.remove("is-drop-target");
  activeEl = el;
  activeEl?.classList.add("is-drop-target");
}

async function ensureInstalled() {
  if (installed) return;
  installed = true;
  const webview = getCurrentWebview();
  await webview.onDragDropEvent((event: any) => {
    const p = event.payload;
    if (p.type === "over" || p.type === "enter") {
      const { zone } = pickZone(p.position.x, p.position.y);
      setActive(zone?.el ?? null);
    } else if (p.type === "leave") {
      setActive(null);
    } else if (p.type === "drop") {
      const { zone, cx, cy } = pickZone(p.position.x, p.position.y);
      setActive(null);
      if (zone) {
        const paths: string[] = Array.isArray(p.paths) ? p.paths : [];
        void zone.handler(paths, { x: cx, y: cy });
      }
    }
  });
}

export function registerDropZone(el: HTMLElement, handler: AssetDropHandler): () => void {
  const z: Zone = { el, handler };
  zones.add(z);
  void ensureInstalled();
  return () => { zones.delete(z); if (activeEl === el) setActive(null); };
}

export function useAssetDrop<T extends HTMLElement>(handler: AssetDropHandler) {
  const ref = useRef<T | null>(null);
  const handlerRef = useRef(handler);
  handlerRef.current = handler;
  useEffect(() => {
    if (!ref.current) return;
    return registerDropZone(ref.current, (paths, pos) => handlerRef.current(paths, pos));
  }, []);
  return ref;
}
