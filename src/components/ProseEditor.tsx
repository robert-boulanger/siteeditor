import { useEditor, EditorContent, Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "tiptap-markdown";
import { useEffect, useRef, useState } from "react";
import { Undo2, Redo2, Image as ImageIcon } from "lucide-react";
import { ResizableImage } from "./ResizableImage";
import { AssetPicker } from "./AssetPicker";
import { useAssetDrop, filterPaths } from "../lib/dropManager";
import { useStore } from "../store";

type Props = {
  /** Initial-Markdown der gerade geladenen Page. Ändert sich, wenn der User die Page wechselt. */
  value: string;
  /** Wird bei jeder Editor-Änderung mit dem aktuellen Markdown aufgerufen. */
  onChange: (markdown: string) => void;
};

/**
 * WYSIWYG-Editor für den Markdown-Body einer Page. Hält intern den
 * ProseMirror-State und serialisiert nach Markdown zurück.
 *
 * Round-Trip-Verträglichkeit: StarterKit deckt Headings, Bold/Italic, Listen,
 * Code, Blockquote, HorizontalRule ab. Tables/Footnotes/Tasklists werden
 * (Phase 05) noch nicht editiert — sie blieben nur erhalten, wenn sie als HTML
 * im Editor stehen. Für die Bootstrap-Inhalte reicht das.
 */
export function ProseEditor({ value, onChange }: Props) {
  const lastEmitted = useRef<string>(value);

  const editor = useEditor({
    extensions: [
      StarterKit,
      ResizableImage.configure({ inline: false, allowBase64: false }),
      Markdown.configure({
        html: true,
        breaks: false,
        linkify: true,
        transformPastedText: true,
        transformCopiedText: true,
      }),
    ],
    content: value,
    onUpdate: ({ editor }) => {
      const md = (editor.storage as any).markdown.getMarkdown() as string;
      lastEmitted.current = md;
      onChange(md);
    },
  });

  // Externe value-Änderungen (z.B. Page-Wechsel) in den Editor spiegeln,
  // aber ohne den ProseMirror-State zu zerstören, wenn nur unsere eigene
  // letzte Emission zurückkommt.
  useEffect(() => {
    if (!editor) return;
    if (value === lastEmitted.current) return;
    editor.commands.setContent(value, { emitUpdate: false });
    lastEmitted.current = value;
  }, [value, editor]);

  const importAsset = useStore((s) => s.importAsset);
  const dropRef = useAssetDrop<HTMLDivElement>(async (paths, pos) => {
    if (!editor) return;
    const accepted = filterPaths(paths, "image");
    if (accepted.length === 0) return;
    const coords = editor.view.posAtCoords({ left: pos.x, top: pos.y });
    const insertPos = coords?.pos ?? editor.state.selection.from;
    let chain = editor.chain().focus().setTextSelection(insertPos);
    for (const p of accepted) {
      const rel = await importAsset(p);
      chain = chain.insertContent({
        type: "image",
        attrs: { src: `/assets/${rel}`, alt: "", align: "inline" },
      });
    }
    chain.run();
  });

  return (
    <div className="prose-editor" ref={dropRef}>
      <Toolbar editor={editor} />
      <EditorContent editor={editor} className="prose-editor-surface" />
    </div>
  );
}

function Toolbar({ editor }: { editor: Editor | null }) {
  const [pickerOpen, setPickerOpen] = useState(false);
  if (!editor) return null;
  const btn = (active: boolean, onClick: () => void, label: string, title: string) => (
    <button
      type="button"
      className={active ? "is-active" : ""}
      onMouseDown={(e) => {
        e.preventDefault();
        onClick();
      }}
      title={title}
    >
      {label}
    </button>
  );
  return (
    <div className="prose-toolbar">
      {btn(editor.isActive("bold"), () => editor.chain().focus().toggleBold().run(), "B", "Fett")}
      {btn(editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run(), "I", "Kursiv")}
      {btn(editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run(), "S", "Durchgestrichen")}
      <span className="sep" />
      {btn(
        editor.isActive("heading", { level: 1 }),
        () => editor.chain().focus().toggleHeading({ level: 1 }).run(),
        "H1",
        "Überschrift 1",
      )}
      {btn(
        editor.isActive("heading", { level: 2 }),
        () => editor.chain().focus().toggleHeading({ level: 2 }).run(),
        "H2",
        "Überschrift 2",
      )}
      {btn(
        editor.isActive("heading", { level: 3 }),
        () => editor.chain().focus().toggleHeading({ level: 3 }).run(),
        "H3",
        "Überschrift 3",
      )}
      <span className="sep" />
      {btn(
        editor.isActive("bulletList"),
        () => editor.chain().focus().toggleBulletList().run(),
        "• Liste",
        "Aufzählung",
      )}
      {btn(
        editor.isActive("orderedList"),
        () => editor.chain().focus().toggleOrderedList().run(),
        "1. Liste",
        "Nummerierte Liste",
      )}
      {btn(editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run(), "❝", "Zitat")}
      {btn(editor.isActive("code"), () => editor.chain().focus().toggleCode().run(), "‹›", "Code (inline)")}
      {btn(editor.isActive("codeBlock"), () => editor.chain().focus().toggleCodeBlock().run(), "{ }", "Code-Block")}
      <span className="sep" />
      <button
        type="button"
        onMouseDown={(e) => { e.preventDefault(); setPickerOpen(true); }}
        title="Bild einfügen"
      >
        <ImageIcon size={14} />
      </button>
      {pickerOpen && (
        <AssetPicker
          mode="single" accept="image"
          onCancel={() => setPickerOpen(false)}
          onPick={(paths) => {
            setPickerOpen(false);
            const src = paths[0];
            if (src) editor.chain().focus().insertContent({
              type: "image", attrs: { src, alt: "", align: "inline" },
            }).run();
          }}
        />
      )}
      <span className="sep" />
      <button
        type="button"
        onMouseDown={(e) => { e.preventDefault(); editor.chain().focus().undo().run(); }}
        title="Rückgängig"
      >
        <Undo2 size={14} />
      </button>
      <button
        type="button"
        onMouseDown={(e) => { e.preventDefault(); editor.chain().focus().redo().run(); }}
        title="Wiederholen"
      >
        <Redo2 size={14} />
      </button>
    </div>
  );
}
