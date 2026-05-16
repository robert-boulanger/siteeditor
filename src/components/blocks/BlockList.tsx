import { useRef, useState } from "react";
import { ChevronDown, ChevronRight, ArrowUp, ArrowDown, Trash2, ChevronsDown, ChevronsUp, GripVertical } from "lucide-react";
import {
  DndContext,
  PointerSensor,
  KeyboardSensor,
  useSensor,
  useSensors,
  closestCenter,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
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

  // Stabile IDs pro Block-Objekt-Identität — bleiben über Reorder erhalten,
  // damit dnd-kit-Animationen sauber laufen. update() ersetzt das Objekt,
  // dann bekommt es eine neue ID — das ist ok, weil dabei keine DnD-Animation läuft.
  const idMap = useRef(new WeakMap<Block, string>());
  const idCounter = useRef(0);
  const idOf = (b: Block): string => {
    let id = idMap.current.get(b);
    if (!id) {
      id = `b${++idCounter.current}`;
      idMap.current.set(b, id);
    }
    return id;
  };
  const ids = blocks.map(idOf);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const onDragEnd = (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const from = ids.indexOf(String(active.id));
    const to = ids.indexOf(String(over.id));
    if (from < 0 || to < 0) return;
    onChange(arrayMove(blocks, from, to));
    setOpenIdxs((prev) => {
      const next = new Set<number>();
      for (const i of prev) {
        if (i === from) next.add(to);
        else if (from < to && i > from && i <= to) next.add(i - 1);
        else if (from > to && i >= to && i < from) next.add(i + 1);
        else next.add(i);
      }
      return next;
    });
  };

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
    // Sortier-ID auf das neue Block-Objekt vererben, sonst bekommt der <li>
    // einen neuen React-Key und TipTap im Text-Block wird bei jeder
    // Tastatureingabe ge-remountet (Cursor weg, Eingabe verloren).
    const prevId = idMap.current.get(blocks[idx]);
    if (prevId) idMap.current.set(b, prevId);
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
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          <ol>
            {blocks.map((b, i) => (
              <SortableBlockItem
                key={ids[i]}
                id={ids[i]}
                block={b}
                index={i}
                total={blocks.length}
                isOpen={openIdxs.has(i)}
                onToggle={() => toggle(i)}
                onMove={(dir) => move(i, dir)}
                onRemove={() => remove(i)}
                onUpdate={(nb) => update(i, nb)}
              />
            ))}
          </ol>
        </SortableContext>
      </DndContext>
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

type SortableBlockItemProps = {
  id: string;
  block: Block;
  index: number;
  total: number;
  isOpen: boolean;
  onToggle: () => void;
  onMove: (dir: -1 | 1) => void;
  onRemove: () => void;
  onUpdate: (b: Block) => void;
};

function SortableBlockItem({
  id, block, index, total, isOpen, onToggle, onMove, onRemove, onUpdate,
}: SortableBlockItemProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : undefined,
  };
  return (
    <li ref={setNodeRef} style={style} className={isOpen ? "is-open" : ""}>
      <div className="block-head">
        {isOpen ? (
          <span className="block-drag is-disabled" title="Block zum Verschieben einklappen">
            <GripVertical size={14} />
          </span>
        ) : (
          <button
            type="button"
            className="block-drag"
            title="Zum Verschieben ziehen"
            aria-label="Drag handle"
            {...attributes}
            {...listeners}
          >
            <GripVertical size={14} />
          </button>
        )}
        <button className="block-type" onClick={onToggle} title={isOpen ? "Einklappen" : "Ausklappen"}>
          {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <span className="badge">{block.type}</span>
          <span className="hint">{blockSummary(block)}</span>
        </button>
        <span className="block-actions">
          <button type="button" title="Nach oben verschieben" onClick={() => onMove(-1)} disabled={index === 0}>
            <ArrowUp size={14} />
          </button>
          <button type="button" title="Nach unten verschieben" onClick={() => onMove(1)} disabled={index === total - 1}>
            <ArrowDown size={14} />
          </button>
          <button type="button" title="Block löschen" onClick={onRemove}>
            <Trash2 size={14} />
          </button>
        </span>
      </div>
      {isOpen && (
        <div className="block-body">
          <BlockForm block={block} onChange={onUpdate} />
        </div>
      )}
    </li>
  );
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
