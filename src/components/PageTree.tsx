import { Eye, EyeOff, Pencil, Trash2, Star } from "lucide-react";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { useState } from "react";
import type { PageSummary } from "../store";

type TreeNode = PageSummary & { children: TreeNode[]; order: number };

/** Build a tree from the flat page list. Parent is the nearest existing
 *  slug prefix; orphans become roots. Siblings sorted by frontend-known
 *  proxy order (we don't have menu.order in PageSummary — fall back to
 *  alphabetical, which matches backend tie-break). */
export function buildPageTree(pages: PageSummary[]): TreeNode[] {
  const bySlug = new Map<string, TreeNode>();
  for (const p of pages) bySlug.set(p.slug, { ...p, children: [], order: 0 });

  const slugsByDepthDesc = [...bySlug.keys()].sort(
    (a, b) => b.split("/").length - a.split("/").length,
  );
  for (const slug of slugsByDepthDesc) {
    const parent = nearestAncestor(slug, bySlug);
    if (parent) bySlug.get(parent)!.children.unshift(bySlug.get(slug)!);
  }
  const roots: TreeNode[] = [];
  for (const [slug, node] of bySlug) {
    if (!nearestAncestor(slug, bySlug)) roots.push(node);
  }
  // Sortierung muss zur Backend-Menü-Reihenfolge passen:
  // primär nach menu_order (default 1000), Tiebreak alphabetisch nach Titel.
  function sortRec(nodes: TreeNode[]) {
    nodes.sort((a, b) => {
      const ao = a.menu_order ?? 1000;
      const bo = b.menu_order ?? 1000;
      if (ao !== bo) return ao - bo;
      return a.title.localeCompare(b.title);
    });
    for (const n of nodes) sortRec(n.children);
  }
  sortRec(roots);
  return roots;
}

function nearestAncestor(slug: string, map: Map<string, TreeNode>): string | null {
  let i = slug.lastIndexOf("/");
  while (i >= 0) {
    const parent = slug.substring(0, i);
    if (map.has(parent)) return parent;
    i = parent.lastIndexOf("/");
  }
  return null;
}

function parentOf(slug: string): string | null {
  const i = slug.lastIndexOf("/");
  return i < 0 ? null : slug.substring(0, i);
}

/** Page-Tree mit DnD. Drop-Zonen pro Knoten: oben (insert before),
 *  unten (insert after), Mitte (become child). Drop in den Root-Spacer
 *  oben = promote to root. */
export function PageTree(props: {
  nodes: TreeNode[];
  currentSlug?: string;
  onSelect: (slug: string) => void;
  onToggleVisible: (slug: string) => void;
  onToggleFavorite: (slug: string, favorite: boolean) => void;
  onRename: (slug: string) => void;
  onDelete: (slug: string) => void;
  onMove: (slug: string, newParent: string | null, newOrder: number | null) => void;
  /** Wenn true: nur flach rendern (für Favoriten-Sektion), DnD-Drop-Zonen aus. */
  flat?: boolean;
}) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
  const [draggedSlug, setDraggedSlug] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  function toggle(slug: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(slug)) next.delete(slug); else next.add(slug);
      return next;
    });
  }

  function handleEnd(event: DragEndEvent) {
    setDraggedSlug(null);
    const active = String(event.active.id);
    const over = event.over ? String(event.over.id) : null;
    if (!over) return;

    // Drag-ID = slug; Drop-ID = "kind:slug" mit kind ∈ {before, after, child, root}
    const draggedSlug = active;
    const [kind, targetSlug] = over.split(":", 2);

    // No self-drop, no drop into own descendant
    if (kind !== "root") {
      if (targetSlug === draggedSlug) return;
      if (targetSlug.startsWith(`${draggedSlug}/`)) return;
    }

    if (kind === "child") {
      // Reparent: target wird neuer Parent, am Ende
      if (parentOf(draggedSlug) === targetSlug) return;
      props.onMove(draggedSlug, targetSlug, null);
      return;
    }
    if (kind === "root") {
      // Sehr niedrige Order, damit die Page wirklich oben landet
      // (sonst sortiert das Backend alphabetisch).
      props.onMove(draggedSlug, null, -10000);
      return;
    }
    // before / after
    const newParent = parentOf(targetSlug);
    // Order-Heuristik: simpler Bias, Tiebreak im Backend alphabetisch.
    // Wir kennen die echten menu.order-Werte hier nicht (PageSummary trägt sie
    // nicht), darum nutzen wir den Slug-Index als Proxy: vorher = niedriger.
    const order = kind === "before" ? -1000 : 1000;
    props.onMove(draggedSlug, newParent, order);
  }

  return (
    <DndContext
      sensors={sensors}
      onDragStart={(e) => setDraggedSlug(String(e.active.id))}
      onDragCancel={() => setDraggedSlug(null)}
      onDragEnd={handleEnd}
    >
      <ul className="page-tree">
        {!props.flat && <RootDropZone />}
        {props.nodes.map((n) => (
          <PageTreeNode
            key={n.slug}
            node={n}
            depth={0}
            draggedSlug={draggedSlug}
            collapsed={collapsed}
            onToggleCollapsed={toggle}
            {...props}
          />
        ))}
      </ul>
      <DragOverlay>
        {draggedSlug && (
          <div className="page-tree-drag-ghost">{draggedSlug}</div>
        )}
      </DragOverlay>
    </DndContext>
  );
}

function RootDropZone() {
  const { setNodeRef, isOver } = useDroppable({ id: "root:" });
  return (
    <li
      ref={setNodeRef}
      className={`page-tree-root-drop${isOver ? " is-over" : ""}`}
      aria-label="Hierher ziehen zum Hochstufen auf Root"
    />
  );
}

function PageTreeNode({
  node,
  depth,
  currentSlug,
  draggedSlug,
  collapsed,
  onToggleCollapsed,
  onSelect,
  onToggleVisible,
  onToggleFavorite,
  onRename,
  onDelete,
  onMove,
  flat,
}: {
  node: TreeNode;
  depth: number;
  currentSlug?: string;
  draggedSlug: string | null;
  collapsed: Set<string>;
  onToggleCollapsed: (slug: string) => void;
  onSelect: (slug: string) => void;
  onToggleVisible: (slug: string) => void;
  onToggleFavorite: (slug: string, favorite: boolean) => void;
  onRename: (slug: string) => void;
  onDelete: (slug: string) => void;
  onMove: (slug: string, newParent: string | null, newOrder: number | null) => void;
  flat?: boolean;
}) {
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsed.has(node.slug);
  const label = node.slug.includes("/") ? node.slug.split("/").pop()! : node.slug;
  const isActive = currentSlug === node.slug;
  const isDragging = draggedSlug === node.slug;

  // Drag-Source (whole row) — kein "handle"-Listener auf den Action-Buttons.
  const drag = useDraggable({ id: node.slug });

  // Drop-Zonen
  const before = useDroppable({ id: `before:${node.slug}` });
  const after = useDroppable({ id: `after:${node.slug}` });
  const child = useDroppable({ id: `child:${node.slug}` });

  // Drop-Targets ausblenden, die sinnlos wären
  const droppingDisabled = draggedSlug != null && (
    draggedSlug === node.slug || node.slug.startsWith(`${draggedSlug}/`)
  );

  return (
    <li className={`page-tree-node${isActive ? " is-active" : ""}${isDragging ? " is-dragging" : ""}`}>
      {!droppingDisabled && !flat && (
        <div
          ref={before.setNodeRef}
          className={`page-tree-gap${before.isOver ? " is-over" : ""}`}
          style={{ marginLeft: `${0.4 + depth * 0.9}rem` }}
        />
      )}
      <div
        ref={flat ? undefined : child.setNodeRef}
        className={`page-tree-row${child.isOver && !droppingDisabled && !flat ? " is-drop-target" : ""}`}
        style={{ paddingLeft: `${flat ? 0.4 : 0.4 + depth * 0.9}rem` }}
      >
        {hasChildren ? (
          <button
            className="tree-toggle"
            title={isCollapsed ? "Einblenden" : "Ausblenden"}
            onClick={(e) => { e.stopPropagation(); onToggleCollapsed(node.slug); }}
            aria-expanded={!isCollapsed}
          >
            {isCollapsed ? "▸" : "▾"}
          </button>
        ) : (
          <span className="tree-toggle tree-toggle--empty" aria-hidden />
        )}
        <button
          ref={drag.setNodeRef}
          {...drag.listeners}
          {...drag.attributes}
          className="page-select"
          onClick={() => onSelect(node.slug)}
          title={node.slug}
        >
          <span className="title">{node.title}</span>
          <span className="slug muted">{label}</span>
          {!node.visible && <span className="badge">hidden</span>}
        </button>
        <span className="page-actions">
          <button
            className={node.favorite ? "fav is-on" : "fav"}
            title={node.favorite ? "Favorit entfernen" : "Als Favorit markieren"}
            onClick={() => onToggleFavorite(node.slug, !node.favorite)}
          >
            <Star size={14} fill={node.favorite ? "currentColor" : "none"} />
          </button>
          <button title={node.visible ? "Page ausblenden" : "Page einblenden"} onClick={() => onToggleVisible(node.slug)}>
            {node.visible ? <Eye size={14} /> : <EyeOff size={14} />}
          </button>
          <button title="Umbenennen" onClick={() => onRename(node.slug)}>
            <Pencil size={14} />
          </button>
          <button title="Löschen" onClick={() => onDelete(node.slug)}>
            <Trash2 size={14} />
          </button>
        </span>
      </div>
      {hasChildren && !isCollapsed && (
        <ul className="page-tree-children">
          {node.children.map((c) => (
            <PageTreeNode
              key={c.slug}
              node={c}
              depth={depth + 1}
              currentSlug={currentSlug}
              draggedSlug={draggedSlug}
              collapsed={collapsed}
              onToggleCollapsed={onToggleCollapsed}
              onSelect={onSelect}
              onToggleVisible={onToggleVisible}
              onToggleFavorite={onToggleFavorite}
              onRename={onRename}
              onDelete={onDelete}
              onMove={onMove}
            />
          ))}
        </ul>
      )}
      {!droppingDisabled && !flat && (
        <div
          ref={after.setNodeRef}
          className={`page-tree-gap${after.isOver ? " is-over" : ""}`}
          style={{ marginLeft: `${0.4 + depth * 0.9}rem` }}
        />
      )}
    </li>
  );
}
