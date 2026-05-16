import Image from "@tiptap/extension-image";
import { ReactNodeViewRenderer, NodeViewWrapper, type NodeViewProps } from "@tiptap/react";
import { NodeSelection, TextSelection } from "@tiptap/pm/state";
import { useEffect, useRef, useState } from "react";
import { useStore } from "../store";
import { AlignLeft, AlignCenter, AlignRight, WrapText, Maximize2, Trash2, ArrowUp, ArrowDown } from "lucide-react";

/**
 * Erweitert die Standard-Image-Node um:
 *  - `width` (px) und `align` (left | center | right | inline) als Attribute
 *  - Drag-Resize-Griff unten rechts
 *  - kleine Toolbar zum Umschalten der Ausrichtung
 *
 * Serialisierung als `<img src width align>` mit inline style → bleibt durch
 * pulldown-cmark beim Build erhalten (HTML pass-through).
 */
export const ResizableImage = Image.extend({
  draggable: true,
  selectable: true,

  addAttributes() {
    const parent = this.parent?.() ?? {};
    return {
      ...parent,
      width: {
        default: null,
        parseHTML: (el) => {
          const w = el.getAttribute("width") || el.style.width;
          if (!w) return null;
          const n = parseInt(w, 10);
          return Number.isFinite(n) ? n : null;
        },
        renderHTML: (attrs) => (attrs.width ? { width: attrs.width } : {}),
      },
      align: {
        default: "inline",
        parseHTML: (el) => el.getAttribute("data-align") || "inline",
        renderHTML: (attrs) => {
          const a = attrs.align ?? "inline";
          let style = "";
          if (a === "left") style = "float:left;margin:0 1rem 0.5rem 0;";
          else if (a === "right") style = "float:right;margin:0 0 0.5rem 1rem;";
          else if (a === "center") style = "display:block;margin:0.5rem auto;";
          return { "data-align": a, style };
        },
      },
    };
  },

  addNodeView() {
    return ReactNodeViewRenderer(ImageNodeView);
  },

  // tiptap-markdown serialisiert Image-Nodes per Default als `![]()` und
  // verliert dadurch width/align. Wir geben rohes HTML aus, damit
  // pulldown-cmark das im Build 1:1 durchreicht.
  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: any) {
          const { src, alt, title, width, align } = node.attrs;
          const attrs: string[] = [`src="${escapeAttr(src ?? "")}"`];
          if (alt) attrs.push(`alt="${escapeAttr(alt)}"`);
          if (title) attrs.push(`title="${escapeAttr(title)}"`);
          if (width) attrs.push(`width="${width}"`);
          if (align && align !== "inline") {
            attrs.push(`data-align="${align}"`);
            let style = "";
            if (align === "left") style = "float:left;margin:0 1rem 0.5rem 0;";
            else if (align === "right") style = "float:right;margin:0 0 0.5rem 1rem;";
            else if (align === "center") style = "display:block;margin:0.5rem auto;";
            if (style) attrs.push(`style="${style}"`);
          }
          state.write(`<img ${attrs.join(" ")}>`);
          state.closeBlock(node);
        },
        parse: { /* parsing übernimmt parseHTML der Attribute */ },
      },
    };
  },
});

function moveNode(editor: any, getPos: any, dir: -1 | 1) {
  if (!editor || typeof getPos !== "function") return;
  const pos = getPos();
  if (typeof pos !== "number") return;
  const { state, view } = editor;
  const $pos = state.doc.resolve(pos);
  const parent = $pos.parent;
  const index = $pos.index();
  const target = index + dir;
  if (target < 0 || target >= parent.childCount) return;
  const node = parent.child(index);
  const sibling = parent.child(target);
  let tr = state.tr.delete(pos, pos + node.nodeSize);
  const insertPos = dir < 0 ? pos - sibling.nodeSize : pos + sibling.nodeSize;
  tr = tr.insert(insertPos, node);
  try { tr = tr.setSelection(NodeSelection.create(tr.doc, insertPos)); } catch { /* ok */ }
  view.dispatch(tr);
  view.focus();
}

/** Setzt align=left|right und sorgt dafür, dass DANACH ein Absatz steht,
 *  in dem der User den umfließenden Text tippen kann. Cursor landet in
 *  diesem Absatz. */
function setAlignWithWrap(editor: any, getPos: any, updateAttributes: any, align: "left" | "right") {
  updateAttributes({ align });
  if (!editor || typeof getPos !== "function") return;
  const pos = getPos();
  if (typeof pos !== "number") return;
  const { state, view } = editor;
  const node = state.doc.nodeAt(pos);
  if (!node) return;
  const after = pos + node.nodeSize;
  const next = state.doc.nodeAt(after);
  const paragraphType = state.schema.nodes.paragraph;
  if (!paragraphType) return;
  let tr = state.tr;
  let targetPos = after + 1;
  if (!next || next.type.name !== "paragraph") {
    tr = tr.insert(after, paragraphType.create());
    targetPos = after + 1;
  }
  try { tr = tr.setSelection(TextSelection.near(tr.doc.resolve(targetPos))); } catch { /* ok */ }
  view.dispatch(tr);
  view.focus();
}

function deleteNode(editor: any, getPos: any) {
  if (!editor || typeof getPos !== "function") return;
  const pos = getPos();
  if (typeof pos !== "number") return;
  const node = editor.state.doc.nodeAt(pos);
  if (!node) return;
  editor.view.dispatch(editor.state.tr.delete(pos, pos + node.nodeSize));
}

function escapeAttr(v: string): string {
  return String(v).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

type Align = "left" | "center" | "right" | "inline" | "full";

function ImageNodeView({ node, updateAttributes, selected, editor, getPos }: NodeViewProps) {
  const { src, alt, width, align } = node.attrs as {
    src: string; alt?: string; width: number | null; align: Align;
  };
  const imgRef = useRef<HTMLImageElement | null>(null);
  const [dragging, setDragging] = useState(false);
  const [resolvedSrc, setResolvedSrc] = useState<string>(src);
  const readAssetDataUrl = useStore((s) => s.readAssetDataUrl);

  // /assets/foo.jpg ist im Tauri-Webview unter http://localhost:1420 nicht
  // erreichbar — wir lösen lazy auf data-URL auf, ändern aber node.attrs.src
  // NICHT (im Markdown soll der Asset-Pfad bleiben, sonst kommen riesige
  // base64-Blobs in die Page).
  useEffect(() => {
    let cancelled = false;
    if (src && src.startsWith("/assets/")) {
      const rel = src.replace(/^\/assets\//, "");
      readAssetDataUrl(rel).then((url) => { if (!cancelled) setResolvedSrc(url); })
        .catch(() => { /* broken-image bleibt sichtbar */ });
    } else {
      setResolvedSrc(src);
    }
    return () => { cancelled = true; };
  }, [src, readAssetDataUrl]);

  function startResize(e: React.PointerEvent) {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = imgRef.current?.getBoundingClientRect().width ?? (width ?? 200);
    setDragging(true);
    function onMove(ev: PointerEvent) {
      const next = Math.max(40, Math.round(startW + (ev.clientX - startX)));
      updateAttributes({ width: next });
    }
    function onUp() {
      setDragging(false);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    }
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  const wrapperStyle: React.CSSProperties =
    align === "left"  ? { float: "left",  margin: "0 1rem 0.5rem 0", maxWidth: "60%" } :
    align === "right" ? { float: "right", margin: "0 0 0.5rem 1rem", maxWidth: "60%" } :
    align === "center"? { display: "block", margin: "0.5rem auto", textAlign: "center" } :
    align === "full"  ? { display: "block", margin: "0.5rem 0", width: "100%" } :
    { display: "inline-block" };

  return (
    <NodeViewWrapper
      as="div"
      className={`resizable-image ${selected ? "is-selected" : ""}`}
      style={wrapperStyle}
      draggable="true"
      data-drag-handle
    >
      <span className="resizable-image-inner" contentEditable={false} style={{ position: "relative", display: "inline-block" }}>
        <img
          ref={imgRef}
          src={resolvedSrc}
          alt={alt ?? ""}
          width={width ?? undefined}
          draggable={false}
          style={{ display: "block", maxWidth: "100%", height: "auto", userSelect: "none" }}
        />
        {selected && (
          <>
            <span className="resize-handle" onPointerDown={startResize}
              style={{ cursor: "nwse-resize", opacity: dragging ? 0.9 : 0.7 }} />
            <span className="image-align-toolbar" contentEditable={false}>
              <button type="button" title="Nach oben verschieben"
                onMouseDown={(e) => { e.preventDefault(); moveNode(editor, getPos, -1); }}>
                <ArrowUp size={14} />
              </button>
              <button type="button" title="Nach unten verschieben"
                onMouseDown={(e) => { e.preventDefault(); moveNode(editor, getPos, +1); }}>
                <ArrowDown size={14} />
              </button>
              <span className="sep" />
              <button type="button" className={align === "left" ? "is-active" : ""}
                title="Links, Text fließt rechts daneben"
                onMouseDown={(e) => { e.preventDefault(); setAlignWithWrap(editor, getPos, updateAttributes, "left"); }}>
                <span style={{ position: "relative", display: "inline-flex" }}>
                  <AlignLeft size={14} />
                  <WrapText size={9} style={{ position: "absolute", right: -4, bottom: -3 }} />
                </span>
              </button>
              <button type="button" className={align === "right" ? "is-active" : ""}
                title="Rechts, Text fließt links daneben"
                onMouseDown={(e) => { e.preventDefault(); setAlignWithWrap(editor, getPos, updateAttributes, "right"); }}>
                <span style={{ position: "relative", display: "inline-flex" }}>
                  <AlignRight size={14} />
                  <WrapText size={9} style={{ position: "absolute", left: -4, bottom: -3 }} />
                </span>
              </button>
              <button type="button" className={align === "center" ? "is-active" : ""}
                title="Zentriert (eigene Zeile)"
                onMouseDown={(e) => { e.preventDefault(); updateAttributes({ align: "center" }); }}>
                <AlignCenter size={14} />
              </button>
              <button type="button" className={align === "full" ? "is-active" : ""}
                title="Volle Breite"
                onMouseDown={(e) => { e.preventDefault(); updateAttributes({ align: "full" }); }}>
                <Maximize2 size={14} />
              </button>
              <span className="sep" />
              <button type="button" title="Bild entfernen"
                onMouseDown={(e) => { e.preventDefault(); deleteNode(editor, getPos); }}>
                <Trash2 size={14} />
              </button>
            </span>
          </>
        )}
      </span>
    </NodeViewWrapper>
  );
}
