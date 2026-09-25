import { invoke } from "@tauri-apps/api/core";

export type Theme = "system" | "light" | "dark";
export type SavePathMode = "copy" | "move";
export type OverwriteStrategy = "overwrite" | "skip";

export interface ExcludeRules {
  hidden: string[];
  risk: string[];
}

export interface SavePathEntry {
  id: string;
  name: string | null;
  path: string;
}

export interface AppConfig {
  schemaVersion: number;
  theme: Theme;
  savePathMode: SavePathMode;
  showHiddenEntries: boolean;
  excludeRules: ExcludeRules;
  savePaths: SavePathEntry[];
}

export interface FileEntry {
  name: string;
  size: number;
  hidden: boolean;
  risk: boolean;
}

export interface ChapterMetaEntry {
  note?: string | null;
}

export interface ChapterInfo {
  number: number;
  path: string;
  fileCount: number;
  meta: ChapterMetaEntry | null;
}

export interface PathStatus {
  id: string;
  exists: boolean;
  isDirectory: boolean;
}

export type RelocateResult =
  | { kind: "success"; newPath: string; candidatesTried: string[] }
  | { kind: "notFound"; candidatesTried: string[] };

export interface LoadResult {
  copied: number;
  overwritten: number;
  skipped: number;
}

export const api = {
  loadConfig: () => invoke<AppConfig>("load_config"),
  saveConfig: (config: AppConfig) => invoke<void>("save_config", { config }),

  addSavePath: (path: string, name: string | null) =>
    invoke<SavePathEntry>("add_save_path", { path, name }),
  removeSavePath: (id: string) => invoke<void>("remove_save_path", { id }),
  validateSavePaths: () => invoke<PathStatus[]>("validate_save_paths"),
  relocateSavePath: (id: string, userSelected: string) =>
    invoke<RelocateResult>("relocate_save_path", { id, userSelected }),

  listSaveFiles: (id: string) => invoke<FileEntry[]>("list_save_files", { id }),
  listChapters: (id: string) => invoke<ChapterInfo[]>("list_chapters", { id }),
  createChapter: (
    id: string,
    files: string[],
    mode: SavePathMode,
    note: string | null,
  ) => invoke<ChapterInfo>("create_chapter", { id, files, mode, note }),
  loadChapter: (id: string, number: number, strategy: OverwriteStrategy) =>
    invoke<LoadResult>("load_chapter", { id, number, strategy }),
  deleteChapter: (id: string, number: number) =>
    invoke<void>("delete_chapter", { id, number }),
  updateChapterNote: (id: string, number: number, note: string | null) =>
    invoke<void>("update_chapter_note", { id, number, note }),

  trashFiles: (id: string, files: string[]) =>
    invoke<void>("trash_files", { id, files }),

  watchSavePath: (id: string | null) =>
    invoke<void>("watch_save_path", { id }),

  revealInExplorer: (path: string, select: string | null) =>
    invoke<void>("reveal_in_explorer", { path, select }),
  exportConfig: (dest: string) => invoke<void>("export_config", { dest }),
  importConfig: (src: string) => invoke<AppConfig>("import_config", { src }),
  openConfigDir: () => invoke<void>("open_config_dir"),
};