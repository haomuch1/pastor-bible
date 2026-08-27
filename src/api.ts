// The only way this window talks to anything.
//
// Every call goes to a Rust command. There is no fetch, no XMLHttpRequest, and
// no browser storage anywhere in this frontend: settings and history live in
// user.db and are read and written through the commands below. A reader who
// clears their browser data, if there were such a thing here, would lose
// nothing, because nothing is kept on this side.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  Answer,
  AppInfo,
  AppSettings,
  ChapterOut,
  DownloadProgress,
  Hardware,
  HistoryDetail,
  HistoryRow,
  Retrieved,
  SelfTestResult,
  Stage,
  StartupState,
} from "./types";

export const appInfo = () => invoke<AppInfo>("app_info");
export const hardwareCheck = () => invoke<Hardware>("hardware_check");
export const startupState = () => invoke<StartupState>("startup_state");

export const getSettings = () => invoke<AppSettings>("get_settings");
export const setSetting = (key: string, value: string) =>
  invoke<AppSettings>("set_setting", { key, value });

export const downloadModel = (id: string) => invoke<string>("download_model", { id });
export const cancelDownload = () => invoke<void>("cancel_download");

export const ask = (question: string) => invoke<Answer>("ask", { question });
export const cancelAsk = () => invoke<void>("cancel_ask");
export const retrievedPassages = () => invoke<Retrieved | null>("retrieved_passages");

export const runSelfTest = () => invoke<SelfTestResult>("run_self_test");
export const finishFirstRun = () => invoke<void>("finish_first_run");

export const historyList = (limit = 50, offset = 0) =>
  invoke<HistoryRow[]>("history_list", { limit, offset });
export const historySearch = (query: string) =>
  invoke<HistoryRow[]>("history_search", { query });
export const historyGet = (id: number) => invoke<HistoryDetail | null>("history_get", { id });
export const historyDelete = (id: number) => invoke<boolean>("history_delete", { id });
export const historyClear = () => invoke<number>("history_clear");
export const historyExport = (path: string) => invoke<string>("history_export", { path });

/// A whole chapter, for the reading view. The verses come from index.db.
export const chapter = (bookId: number, chapterNumber: number) =>
  invoke<ChapterOut | null>("chapter", { bookId, chapter: chapterNumber });

export const shutdownModels = () => invoke<void>("shutdown_models");

export const onStage = (f: (s: Stage) => void): Promise<UnlistenFn> =>
  listen<Stage>("ask-stage", (e) => f(e.payload));

export const onDownloadProgress = (f: (p: DownloadProgress) => void): Promise<UnlistenFn> =>
  listen<DownloadProgress>("download-progress", (e) => f(e.payload));

// ---- small shared formatting -------------------------------------------

export function bytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(0)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(0)} kB`;
  return `${n} bytes`;
}

export function duration(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) return "";
  const s = Math.round(seconds);
  if (s < 60) return `${s} second${s === 1 ? "" : "s"}`;
  const m = Math.floor(s / 60);
  const r = s % 60;
  if (m < 60) return r === 0 ? `${m} minute${m === 1 ? "" : "s"}` : `${m} min ${r} s`;
  const h = Math.floor(m / 60);
  return `${h} h ${m % 60} min`;
}

export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export function whenAsked(iso: string): string {
  // The stored form is ISO 8601 UTC; show it in the reader's own time.
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
