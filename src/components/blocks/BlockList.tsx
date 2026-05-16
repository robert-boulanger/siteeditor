import { useState } from "react";
import { ChevronDown, ChevronRight, ArrowUp, ArrowDown, Trash2, ChevronsDown, ChevronsUp } from "lucide-react";
import type { Block } from "../../store";
import { BLOCK_TYPES, newBlock } from "../../store";
import { BlockForm } from "./BlockForms";

type Props = {
  blocks: Block[];
  onChange: (blocks: Block[]) => void;
};

const BLOCK_DESCRIPTIONS: Record<Block["type"], string> = {
  hero: "Hero — große Eingangs-Sektion mit Headline, optionalem Untertitel und Bild",
  text: "Text — gibt den Markdown-Body der Page wieder (max. 1 pro Page)",
  image: "Image — Einzelbild mit Bildunterschrift und Breitenoption",
  gallery: "Gallery — mehrere Bilder als Grid oder Masonry",
  video: "Video — eingebettetes Video mit optionaler Bildunterschrift",
  cta: "Call-to-Action — auffälliger Button mit Link",
  columns: "Columns — Layout mit mehreren Spalten, jede mit Sub-Blocks",
  quote: "Quote — Zitat mit Autor und Quelle",
};

export function BlockList({ blocks, onChange }: Props) {
  const [openIdxs, setOpenIdxs] = useState<Set<number>>(new Set());

  const toggle = (i: number) => {
    setOpenIdxs((prev) => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      return next;
    });
  };
  const expandAll = () => setOpenIdxs(new Set(blocks.map((_, i) => i)));
  const collapseAll = () => setOpenIdxs(new Set());

  const update = (idx: number, b: Block) => {
    const next = [...blocks];
    next[idx] = b;
    onChange(next);
  };
  const remove = (idx: number) => {
    onChange(blocks.filter((_, i) => i !== idx));
    setOpenIdxs((prev) => {
      const next = new Set<number>();
      for (const i of prev) {
        if (i < idx) next.add(i);
        else if (i > idx) next.add(i - 1);
      }
      return next;
    });
  };
  const move = (idx: number, dir: -1 | 1) => {
    const target = idx + dir;
    if (target < 0 || target >= blocks.length) return;
    const next = [...blocks];
    [next[idx], next[target]] = [next[target], next[idx]];
    onChange(next);
    setOpenIdxs((prev) => {
      const next = new Set(prev);
      const hadIdx = prev.has(idx);
      const hadTarget = prev.has(target);
      next.delete(idx); next.delete(target);
      if (hadIdx) next.add(target);
      if (hadTarget) next.add(idx);
      return next;
    });
  };
  const add = (type: Block["type"]) => {
    const newIdx = blocks.length;
    onChange([...blocks, newBlock(type)]);
    setOpenIdxs((prev) => new Set(prev).add(newIdx));
  };

  return (
    <div className="block-list">
      {blocks.length > 0 && (
        <div className="block-list-tools">
          <button type="button" title="Alle ausklappen" onClick={expandAll}>
            <ChevronsDown size={14} /> alle auf
          </button>
          <button type="button" title="Alle einklappen" onClick={collapseAll}>
            <ChevronsUp size={14} /> alle zu
          </button>
        </div>
      )}
      <ol>
        {blocks.map((b, i) => {
          const isOpen = openIdxs.has(i);
          return (
            <li key={i} className={isOpen ? "is-open" : ""}>
              <div className="block-head">
                <button className="block-type" onClick={() => toggle(i)} title={isOpen ? "Einklappen" : "Ausklappen"}>
                  {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                  <span className="badge">{b.type}</span>
                  <span className="hint">{blockSummary(b)}</span>
                </button>
                <span className="block-actions">
                  <button type="button" title="Nach oben verschieben" onClick={() => move(i, -1)} disabled={i === 0}>
                    <ArrowUp size={14} />
                  </button>
                  <button type="button" title="Nach unten verschieben" onClick={() => move(i, 1)} disabled={i === blocks.length - 1}>
                    <ArrowDown size={14} />
                  </button>
                  <button type="button" title="Block löschen" onClick={() => remove(i)}>
                    <Trash2 size={14} />
                  </button>
                </span>
              </div>
              {isOpen && (
                <div className="block-body">
                  <BlockForm block={b} onChange={(nb) => update(i, nb)} />
                </div>
              )}
            </li>
          );
        })}
      </ol>
      <BlockPalette onAdd={add} />
    </div>
  );
}

function blockSummary(b: Block): string {
  switch (b.type) {
    case "hero": return b.headline;
    case "text": return "(Body-Markdown der Page)";
    case "image": return b.image || "(kein Pfad)";
    case "gallery": return `${b.images.length} Bilder`;
    case "video": return b.source || "(keine Quelle)";
    case "cta": return `${b.text} → ${b.href}`;
    case "columns": return `${b.columns.length} Spalten`;
    case "quote": return b.text.slice(0, 60);
  }
}

function BlockPalette({ onAdd }: { onAdd: (t: Block["type"]) => void }) {
  return (
    <div className="block-palette">
      <strong>+ Block:</strong>
      {BLOCK_TYPES.map((t) => (
        <button key={t} type="button" onClick={() => onAdd(t)} title={BLOCK_DESCRIPTIONS[t]}>
          {t}
        </button>
      ))}
    </div>
  );
}
