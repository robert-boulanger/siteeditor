import { useState } from "react";
import { Image as ImageIcon, Folder } from "lucide-react";
import type { Block, ColumnsInner } from "../../store";
import { useStore } from "../../store";
import { ProseEditor } from "../ProseEditor";
import { AssetPicker } from "../AssetPicker";
import { useAssetDrop, filterPaths } from "../../lib/dropManager";

type AssetFieldProps = {
  value: string;
  onChange: (v: string) => void;
  accept?: "image" | "video" | "any";
  placeholder?: string;
};

function AssetField({ value, onChange, accept = "image", placeholder }: AssetFieldProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const importAsset = useStore((s) => s.importAsset);
  const dropRef = useAssetDrop<HTMLDivElement>(async (paths) => {
    const accepted = filterPaths(paths, accept);
    if (accepted.length === 0) return;
    const rel = await importAsset(accepted[0]);
    onChange(`/assets/${rel}`);
  });
  return (
    <>
      <div className="asset-input-row" ref={dropRef}>
        <input
          value={value}
          placeholder={placeholder ?? "/assets/…"}
          onChange={(e) => onChange(e.currentTarget.value)}
        />
        <button
          type="button" className="asset-pick-btn"
          title="Aus Assets wählen" onClick={() => setPickerOpen(true)}
        >
          {accept === "image" ? <ImageIcon size={16} /> : <Folder size={16} />}
        </button>
      </div>
      {pickerOpen && (
        <AssetPicker
          mode="single" accept={accept}
          onCancel={() => setPickerOpen(false)}
          onPick={(paths) => { onChange(paths[0]); setPickerOpen(false); }}
        />
      )}
    </>
  );
}

/**
 * Pro Block-Typ ein simples Formular. Felder folgen den im default-Theme
 * (`page.html`) erwarteten Properties. Keine Block-Validierung im Frontend —
 * der Backend-Builder bricht bei Verstößen klar ab.
 */

type Props<T extends Block> = { block: T; onChange: (b: T) => void };

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block-field">
      <span>{label}</span>
      {children}
    </label>
  );
}

export function BlockForm({ block, onChange }: { block: Block; onChange: (b: Block) => void }) {
  switch (block.type) {
    case "hero": return <HeroForm block={block} onChange={onChange} />;
    case "text": return <TextForm block={block} onChange={onChange} />;
    case "image": return <ImageForm block={block} onChange={onChange} />;
    case "gallery": return <GalleryForm block={block} onChange={onChange} />;
    case "video": return <VideoForm block={block} onChange={onChange} />;
    case "cta": return <CtaForm block={block} onChange={onChange} />;
    case "columns": return <ColumnsForm block={block} onChange={onChange} />;
    case "quote": return <QuoteForm block={block} onChange={onChange} />;
  }
}

function HeroForm({ block, onChange }: Props<Extract<Block, { type: "hero" }>>) {
  return (
    <div className="block-form">
      <Field label="Headline">
        <input value={block.headline} onChange={(e) => onChange({ ...block, headline: e.currentTarget.value })} />
      </Field>
      <Field label="Untertitel">
        <input value={block.sub ?? ""} onChange={(e) => onChange({ ...block, sub: e.currentTarget.value || undefined })} />
      </Field>
      <Field label="Ausrichtung">
        <select
          value={block.align ?? "center"}
          onChange={(e) => onChange({ ...block, align: e.currentTarget.value as any })}
        >
          <option value="left">links</option>
          <option value="center">zentriert</option>
          <option value="right">rechts</option>
        </select>
      </Field>
      <Field label="Bild-Pfad (optional)">
        <AssetField
          value={block.image ?? ""}
          onChange={(v) => onChange({ ...block, image: v || undefined })}
        />
      </Field>
      {block.image && (
        <Field label="Bildunterschrift (optional)">
          <input
            value={block.image_caption ?? ""}
            onChange={(e) => onChange({ ...block, image_caption: e.currentTarget.value || undefined })}
          />
        </Field>
      )}
    </div>
  );
}

function TextForm({ block, onChange }: Props<Extract<Block, { type: "text" }>>) {
  return (
    <div className="block-form">
      <ProseEditor
        value={block.content}
        onChange={(content) => onChange({ ...block, content })}
      />
      <Field label="Style (optional)">
        <input
          value={block.style ?? ""}
          onChange={(e) => onChange({ ...block, style: e.currentTarget.value || undefined })}
        />
      </Field>
    </div>
  );
}

function ImageForm({ block, onChange }: Props<Extract<Block, { type: "image" }>>) {
  return (
    <div className="block-form">
      <Field label="Bild-Pfad">
        <AssetField value={block.image} onChange={(v) => onChange({ ...block, image: v })} />
      </Field>
      <Field label="Bildunterschrift">
        <input value={block.caption ?? ""} onChange={(e) => onChange({ ...block, caption: e.currentTarget.value || undefined })} />
      </Field>
      <Field label="Breite">
        <select value={block.width ?? "normal"} onChange={(e) => onChange({ ...block, width: e.currentTarget.value as any })}>
          <option value="normal">normal</option>
          <option value="wide">breit</option>
          <option value="full">vollbild</option>
        </select>
      </Field>
    </div>
  );
}

function GalleryForm({ block, onChange }: Props<Extract<Block, { type: "gallery" }>>) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const importAsset = useStore((s) => s.importAsset);
  const dropRef = useAssetDrop<HTMLDivElement>(async (paths) => {
    const accepted = filterPaths(paths, "image");
    if (accepted.length === 0) return;
    const rels: string[] = [];
    for (const p of accepted) rels.push(await importAsset(p));
    onChange({ ...block, images: [...block.images, ...rels.map((r) => ({ src: `/assets/${r}` }))] });
  });
  const patch = (idx: number, p: Partial<{ src: string; caption: string }>) => {
    const images = block.images.map((img, j) => j === idx ? { ...img, ...p, caption: p.caption === "" ? undefined : (p.caption ?? img.caption) } : img);
    onChange({ ...block, images });
  };
  return (
    <div className="block-form">
      <Field label="Layout">
        <select value={block.layout ?? "grid"} onChange={(e) => onChange({ ...block, layout: e.currentTarget.value as any })}>
          <option value="grid">grid</option>
          <option value="masonry">masonry</option>
        </select>
      </Field>
      <Field label="Spalten">
        <input
          type="number" min={1} max={6}
          value={block.columns ?? 3}
          onChange={(e) => onChange({ ...block, columns: Number(e.currentTarget.value) })}
        />
      </Field>
      <div className="block-subgroup" ref={dropRef}>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <strong>Bilder <span className="muted small">(Dateien hier ablegen erlaubt)</span></strong>
          <button type="button" className="asset-pick-btn" onClick={() => setPickerOpen(true)}>
            <ImageIcon size={14} /> Aus Assets wählen
          </button>
        </div>
        {block.images.map((img, i) => (
          <div key={i} className="gallery-image-row">
            <div className="asset-input-row">
              <input value={img.src} placeholder="/assets/…" onChange={(e) => patch(i, { src: e.currentTarget.value })} />
              <button type="button" onClick={() => onChange({ ...block, images: block.images.filter((_, j) => j !== i) })}>×</button>
            </div>
            <input
              className="gallery-caption"
              placeholder="Bildunterschrift (optional)"
              value={img.caption ?? ""}
              onChange={(e) => patch(i, { caption: e.currentTarget.value })}
            />
          </div>
        ))}
        <button type="button" onClick={() => onChange({ ...block, images: [...block.images, { src: "" }] })}>+ Bild</button>
      </div>
      {pickerOpen && (
        <AssetPicker
          mode="multi" accept="image"
          onCancel={() => setPickerOpen(false)}
          onPick={(paths) => { onChange({ ...block, images: [...block.images, ...paths.map((src) => ({ src }))] }); setPickerOpen(false); }}
        />
      )}
    </div>
  );
}

function VideoForm({ block, onChange }: Props<Extract<Block, { type: "video" }>>) {
  return (
    <div className="block-form">
      <Field label="Video-Pfad">
        <AssetField value={block.source} accept="video" onChange={(v) => onChange({ ...block, source: v })} />
      </Field>
      <Field label="Bildunterschrift">
        <input value={block.caption ?? ""} onChange={(e) => onChange({ ...block, caption: e.currentTarget.value || undefined })} />
      </Field>
      <label className="block-field row">
        <input type="checkbox" checked={!!block.autoplay} onChange={(e) => onChange({ ...block, autoplay: e.currentTarget.checked })} />
        <span>Autoplay (stumm)</span>
      </label>
    </div>
  );
}

function CtaForm({ block, onChange }: Props<Extract<Block, { type: "cta" }>>) {
  return (
    <div className="block-form">
      <Field label="Text">
        <input value={block.text} onChange={(e) => onChange({ ...block, text: e.currentTarget.value })} />
      </Field>
      <Field label="Link">
        <input value={block.href} onChange={(e) => onChange({ ...block, href: e.currentTarget.value })} />
      </Field>
      <Field label="Style">
        <select value={block.style ?? "primary"} onChange={(e) => onChange({ ...block, style: e.currentTarget.value as any })}>
          <option value="primary">primary</option>
          <option value="secondary">secondary</option>
        </select>
      </Field>
    </div>
  );
}

function QuoteForm({ block, onChange }: Props<Extract<Block, { type: "quote" }>>) {
  return (
    <div className="block-form">
      <Field label="Zitat">
        <textarea rows={3} value={block.text} onChange={(e) => onChange({ ...block, text: e.currentTarget.value })} />
      </Field>
      <Field label="Autor">
        <input value={block.author ?? ""} onChange={(e) => onChange({ ...block, author: e.currentTarget.value || undefined })} />
      </Field>
      <Field label="Quelle">
        <input value={block.source ?? ""} onChange={(e) => onChange({ ...block, source: e.currentTarget.value || undefined })} />
      </Field>
    </div>
  );
}

function ColumnsForm({ block, onChange }: Props<Extract<Block, { type: "columns" }>>) {
  // MVP: pro Spalte eine Liste von Sub-Blocks (text|image|cta|quote) als simple Form.
  const updateCol = (ci: number, items: ColumnsInner[]) => {
    const columns = [...block.columns];
    columns[ci] = items;
    onChange({ ...block, columns });
  };
  return (
    <div className="block-form">
      <div className="row">
        <strong>Spalten: {block.columns.length}</strong>
        <button type="button" onClick={() => onChange({ ...block, columns: [...block.columns, [{ type: "text", content: "" }]] })}>
          + Spalte
        </button>
        {block.columns.length > 1 && (
          <button type="button" onClick={() => onChange({ ...block, columns: block.columns.slice(0, -1) })}>
            − Spalte
          </button>
        )}
      </div>
      {block.columns.map((col, ci) => (
        <div key={ci} className="block-subgroup">
          <strong>Spalte {ci + 1}</strong>
          {col.map((inner, ii) => {
            const replaceInner = (rep: ColumnsInner) => {
              const items = [...col]; items[ii] = rep; updateCol(ci, items);
            };
            return (
              <div key={ii} className="inner-block">
                <div className="row">
                  <select
                    value={inner.type}
                    onChange={(e) => {
                      const t = e.currentTarget.value as ColumnsInner["type"];
                      replaceInner(
                        t === "text" ? { type: "text", content: "" }
                        : t === "image" ? { type: "image", image: "" }
                        : t === "cta" ? { type: "cta", text: "", href: "/" }
                        : { type: "quote", text: "" }
                      );
                    }}
                  >
                    <option value="text">text</option>
                    <option value="image">image</option>
                    <option value="cta">cta</option>
                    <option value="quote">quote</option>
                  </select>
                  {inner.type === "image" && (
                    <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "0.3rem" }}>
                      <AssetField value={inner.image} onChange={(v) => replaceInner({ ...inner, image: v })} />
                      <input
                        placeholder="Bildunterschrift (optional)"
                        value={inner.caption ?? ""}
                        onChange={(e) => replaceInner({ ...inner, caption: e.currentTarget.value || undefined })}
                      />
                    </div>
                  )}
                  {inner.type === "cta" && (
                    <>
                      <input placeholder="Text" value={inner.text}
                        onChange={(e) => replaceInner({ ...inner, text: e.currentTarget.value })} />
                      <input placeholder="Link" value={inner.href}
                        onChange={(e) => replaceInner({ ...inner, href: e.currentTarget.value })} />
                    </>
                  )}
                  {inner.type === "quote" && (
                    <input placeholder="Zitat" value={inner.text}
                      onChange={(e) => replaceInner({ ...inner, text: e.currentTarget.value })} />
                  )}
                  <button type="button" title="Sub-Block löschen"
                    onClick={() => updateCol(ci, col.filter((_, j) => j !== ii))}>×</button>
                </div>
                {inner.type === "text" && (
                  <ProseEditor
                    value={inner.content}
                    onChange={(content) => replaceInner({ ...inner, content })}
                  />
                )}
              </div>
            );
          })}
          <button type="button" onClick={() => updateCol(ci, [...col, { type: "text", content: "" }])}>+ Sub-Block</button>
        </div>
      ))}
    </div>
  );
}
