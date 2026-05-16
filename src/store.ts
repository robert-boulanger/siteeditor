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

export type PageDoc = {
  slug: string;
  frontmatter: {
    title: string;
    template?: string | null;
    visible: boolean;
    menu: { show: boolean; order?: number | null };
    blocks: unknown[];
    meta: Record<string, string>;
  };
  body_markdown: string;
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
  savePageBody: (slug: string, body: string) => Promise<void>;
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

  savePageBody: async (slug: string, body: string) => {
    set({ busy: true, status: `Speichere ${slug} …` });
    try {
      const page = await invoke<PageDoc>("save_page_body", { slug, body });
      set({ currentPage: page, status: `Gespeichert: ${slug}` });
    } catch (e) {
      set({ status: `Save-Fehler: ${e}` });
      throw e;
    } finally {
      set({ busy: false });
    }
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
