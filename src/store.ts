import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export type PageSummary = {
  slug: string;
  title: string;
  visible: boolean;
  template: string | null;
};

export type ProjectState = {
  root: string;
  title: string;
  active_theme: string;
  pages: PageSummary[];
};

export type PageFrontmatter = {
  title: string;
  template?: string | null;
  visible: boolean;
  menu: { show: boolean; order?: number | null };
  blocks: Block[];
  meta: Record<string, string>;
};

export type PageDoc = {
  slug: string;
  frontmatter: PageFrontmatter;
  body_markdown: string;
};

/** Block-Katalog v1 — 8 Typen. Felder sind absichtlich locker getypt,
 *  weil die Templates `default()`-Werte tolerieren. */
export type Block =
  | { type: "hero"; headline: string; sub?: string; align?: "left" | "center" | "right"; image?: string; image_caption?: string }
  | { type: "text"; content: string; style?: string }
  | { type: "image"; image: string; caption?: string; width?: "normal" | "wide" | "full" }
  | { type: "gallery"; images: GalleryImage[]; layout?: "grid" | "masonry"; columns?: number }
  | { type: "video"; source: string; autoplay?: boolean; caption?: string }
  | { type: "cta"; text: string; href: string; style?: "primary" | "secondary" }
  | { type: "columns"; columns: ColumnsInner[][] }
  | { type: "quote"; text: string; author?: string; source?: string };

export type GalleryImage = { src: string; caption?: string };

export type ColumnsInner =
  | { type: "text"; content: string }
  | { type: "image"; image: string; caption?: string }
  | { type: "cta"; text: string; href: string }
  | { type: "quote"; text: string };

export const BLOCK_TYPES: Block["type"][] = [
  "hero", "text", "image", "gallery", "video", "cta", "columns", "quote",
];

export function newBlock(type: Block["type"]): Block {
  switch (type) {
    case "hero": return { type, headline: "Neue Überschrift", align: "center" };
    case "text": return { type, content: "" };
    case "image": return { type, image: "" };
    case "gallery": return { type, images: [], layout: "grid", columns: 3 } as Block;
    case "video": return { type, source: "" };
    case "cta": return { type, text: "Mehr erfahren", href: "/" };
    case "columns": return { type, columns: [[{ type: "text", content: "" }], [{ type: "text", content: "" }]] };
    case "quote": return { type, text: "" };
  }
}

export type AssetInfo = {
  path: string;
  name: string;
  size: number;
  mime: string;
  mtime: number;
};

type BuildResult = {
  pages_rendered: number;
  output_dir: string;
  index_file: string;
};

type Store = {
  project: ProjectState | null;
  currentPage: PageDoc | null;
  status: string;
  busy: boolean;
  bootstrap: (path: string) => Promise<void>;
  open: (path: string) => Promise<void>;
  loadPage: (slug: string) => Promise<void>;
  savePageFull: (slug: string, frontmatter: PageFrontmatter, body: string) => Promise<void>;
  createPage: (slug: string, frontmatter: PageFrontmatter, body?: string) => Promise<void>;
  renamePage: (oldSlug: string, newSlug: string) => Promise<void>;
  deletePage: (slug: string) => Promise<void>;
  listAssets: () => Promise<AssetInfo[]>;
  importAsset: (sourcePath: string) => Promise<string>;
  deleteAsset: (path: string) => Promise<void>;
  readAssetDataUrl: (path: string) => Promise<string>;
  build: () => Promise<BuildResult>;
  setStatus: (s: string) => void;
};

export const useStore = create<Store>((set, get) => ({
  project: null,
  currentPage: null,
  status: "Kein Projekt geöffnet.",
  busy: false,

  setStatus: (s) => set({ status: s }),

  bootstrap: async (path: string) => {
    set({ busy: true, status: `Erzeuge Beispielprojekt in ${path} …` });
    try {
      const project = await invoke<ProjectState>("bootstrap_example", { path });
      set({ project, currentPage: null, status: `Projekt erstellt: ${project.title}` });
      if (project.pages[0]) await get().loadPage(project.pages[0].slug);
    } catch (e) {
      set({ status: `Fehler: ${e}` });
    } finally {
      set({ busy: false });
    }
  },

  open: async (path: string) => {
    set({ busy: true, status: `Öffne ${path} …` });
    try {
      const project = await invoke<ProjectState>("open_project", { path });
      set({ project, currentPage: null, status: `Geöffnet: ${project.title}` });
      if (project.pages[0]) await get().loadPage(project.pages[0].slug);
    } catch (e) {
      set({ status: `Fehler: ${e}` });
    } finally {
      set({ busy: false });
    }
  },

  loadPage: async (slug: string) => {
    try {
      const page = await invoke<PageDoc>("load_page", { slug });
      set({ currentPage: page });
    } catch (e) {
      set({ status: `Page-Load-Fehler: ${e}` });
    }
  },

  savePageFull: async (slug, frontmatter, body) => {
    set({ busy: true, status: `Speichere ${slug} …` });
    try {
      const page = await invoke<PageDoc>("save_page_full", {
        args: { slug, frontmatter, body },
      });
      // Page-Liste neu spiegeln (Title kann sich geändert haben)
      const project = get().project;
      if (project) {
        const pages = project.pages.map((p) =>
          p.slug === slug
            ? { ...p, title: page.frontmatter.title, visible: page.frontmatter.visible, template: page.frontmatter.template ?? null }
            : p,
        );
        set({ project: { ...project, pages } });
      }
      set({ currentPage: page, status: `Gespeichert: ${slug}` });
    } catch (e) {
      set({ status: `Save-Fehler: ${e}` });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  createPage: async (slug, frontmatter, body = "") => {
    set({ busy: true, status: `Lege ${slug} an …` });
    try {
      const pages = await invoke<PageSummary[]>("create_page", { args: { slug, frontmatter, body } });
      const project = get().project;
      if (project) set({ project: { ...project, pages } });
      set({ status: `Page angelegt: ${slug}` });
      await get().loadPage(slug);
    } catch (e) {
      set({ status: `Create-Fehler: ${e}` });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  renamePage: async (oldSlug, newSlug) => {
    set({ busy: true, status: `Benenne um: ${oldSlug} → ${newSlug} …` });
    try {
      const pages = await invoke<PageSummary[]>("rename_page", { oldSlug, newSlug });
      const project = get().project;
      if (project) set({ project: { ...project, pages } });
      const current = get().currentPage;
      if (current?.slug === oldSlug) await get().loadPage(newSlug);
      set({ status: `Umbenannt: ${oldSlug} → ${newSlug}` });
    } catch (e) {
      set({ status: `Rename-Fehler: ${e}` });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  deletePage: async (slug) => {
    set({ busy: true, status: `Lösche ${slug} …` });
    try {
      const pages = await invoke<PageSummary[]>("delete_page", { slug });
      const project = get().project;
      if (project) set({ project: { ...project, pages } });
      if (get().currentPage?.slug === slug) {
        const next = pages[0];
        if (next) await get().loadPage(next.slug);
        else set({ currentPage: null });
      }
      set({ status: `Gelöscht: ${slug}` });
    } catch (e) {
      set({ status: `Delete-Fehler: ${e}` });
      throw e;
    } finally {
      set({ busy: false });
    }
  },

  listAssets: async () => {
    return await invoke<AssetInfo[]>("list_assets");
  },

  importAsset: async (sourcePath: string) => {
    return await invoke<string>("import_asset", { source: sourcePath });
  },

  deleteAsset: async (path: string) => {
    await invoke("delete_asset", { path });
  },

  readAssetDataUrl: async (path: string) => {
    return await invoke<string>("read_asset_data_url", { path });
  },

  build: async () => {
    set({ busy: true, status: "Build läuft …" });
    try {
      const r = await invoke<BuildResult>("build_site");
      set({ status: `Build OK — ${r.pages_rendered} Seiten → ${r.output_dir}` });
      return r;
    } catch (e) {
      set({ status: `Build-Fehler: ${e}` });
      throw e;
    } finally {
      set({ busy: false });
    }
  },
}));
