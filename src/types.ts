// The shapes the backend sends. These mirror docs/API.md and the Rust types in
// src-tauri/core/src/api.rs; when one changes the other must.

export interface VerseOut {
  verse_id: number;
  reference: string;
  text: string;
}

export interface PassageOut {
  token: string | null;
  /// The reference as a reader writes it: "1 Kings 3:9", never "1Ki 3:9".
  reference: string;
  verse_ids: number[];
  verses: VerseOut[];
  score: number;
  origins: string[];
  canon: "protestant" | "deutero";
  cited: boolean;
  sent: boolean;
}

// A whole chapter, for reading a cited passage in its place. The verse text
// comes from index.db exactly as the answer's does.
export interface ChapterRef {
  book_id: number;
  chapter: number;
  reference: string;
}

export interface ChapterOut {
  book_id: number;
  book_name: string;
  chapter: number;
  reference: string;
  canon: "protestant" | "deutero";
  verses: VerseOut[];
  previous: ChapterRef | null;
  next: ChapterRef | null;
}

export interface TopicOut {
  topic_id: number;
  heading: string;
  heading_display: string;
  verses: number;
  score: number;
  passage_refs: string[];
}

export interface TopicGroup {
  heading: string;
  heading_display: string;
  topic_id: number | null;
  passage_refs: string[];
}

export interface ViolationOut {
  kind: string;
  text: string;
  reason: string;
  span: [number, number];
}

export interface AttemptOut {
  verdict: string;
  seconds: number;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  violations: ViolationOut[];
}

export interface Timings {
  index_load_seconds: number;
  embed_server_seconds: number;
  embed_seconds: number;
  chat_server_seconds: number;
  retrieve_seconds: number;
  generate_seconds: number;
  retry_seconds: number;
  verify_seconds: number;
  total_seconds: number;
}

export interface Answer {
  question: string;
  canon_mode: string;
  crisis: boolean;
  crisis_note: string | null;
  synopsis_markdown: string | null;
  fallback_markdown: string | null;
  verdict: string;
  attempts: AttemptOut[];
  fallback_used: boolean;
  cited_tokens: string[];
  cited_passage_ids: number[];
  deuterocanon_cited: boolean;
  deuterocanon_footer: string | null;
  passages: PassageOut[];
  sent_count: number;
  topics: TopicOut[];
  topic_groups: TopicGroup[];
  timings: Timings;
  model_id: string;
  embedding_model_id: string;
  index_version: string;
  prompt_versions: [string, string][];
  sidecar_path: string;
  peak_ram_mb: number | null;
  query_mode: string;
}

// What the reader is waiting for. The tagged union the backend emits.
export type Stage =
  | { stage: "loading_model"; role: string; model: string }
  | { stage: "retrieving" }
  | { stage: "retrieved"; passages: number; sent: number }
  | { stage: "generating"; tokens: number; attempt: number }
  | { stage: "checking_references"; attempt: number }
  | { stage: "retrying"; reason: string }
  | { stage: "done"; verdict: string }
  | { stage: "cancelled" }
  | { stage: "failed"; message: string };

export type DownloadProgress =
  | { stage: "checking"; file: string }
  | {
      stage: "downloading";
      file: string;
      done: number;
      total: number;
      percent: number;
      bytes_per_second: number;
      eta_seconds: number | null;
      resumed_from: number;
    }
  | { stage: "verifying"; file: string; done: number; total: number; percent: number }
  | { stage: "done"; file: string; bytes: number; skipped: boolean }
  | { stage: "failed"; file: string; message: string };

export interface ModelStatus {
  id: string;
  file: string;
  label: string;
  note: string;
  bytes: number;
  bundled: boolean;
  present: boolean;
  partial_bytes: number;
}

export interface Reference {
  cpu: string;
  gpu: string;
  ram_gb: number;
  os: string;
  disk_gb: number;
}

export interface Hardware {
  cpu: string;
  cores: number;
  gpu: string;
  ram_gb: number;
  free_ram_gb: number;
  free_disk_gb: number;
  os: string;
  reference: Reference;
  below: string[];
  warning: string | null;
}

export interface AppSettings {
  canon: string;
  model: string;
  compute: string;
}

// Which processor will answer, and why. `mode` is what Settings asks for;
// `using` is what will actually run.
export interface GpuDevice {
  id: string;
  name: string;
  total_mib: number;
  free_mib: number;
}

export interface ComputeChoice {
  mode: string;
  using: "cpu" | "gpu";
  device: GpuDevice | null;
  needs_mib: number;
  reason: string;
}

export interface SelfTestQuestion {
  id: string;
  question: string;
  verdict: string;
  cited: number;
  sent: number;
  fabrications: number;
  seconds: number;
  ok: boolean;
}

export interface SelfTestResult {
  questions: SelfTestQuestion[];
  passed: boolean;
  seconds: number;
  ran_at: string;
  model_id: string;
  index_version: string;
}

export interface StartupState {
  first_run: boolean;
  /// A plain sentence naming a model file that is missing or wrong, or null.
  model_problem: string | null;
  models: ModelStatus[];
  chat_model_present: boolean;
  embedding_model_present: boolean;
  settings: AppSettings;
  self_test: SelfTestResult | null;
  history_count: number;
}

export interface AppPaths {
  app_data: string;
  user_db: string;
  models: string;
  index_db: string;
  llama_server: string;
  logs: string;
}

export interface AppInfo {
  app_version: string;
  index_version: string;
  model_id: string;
  model_file: string;
  embedding_model: string;
  disclaimer: string;
  crisis_note: string;
  offline_statement: string;
  authors: string[];
  license: string;
  sources: [string, string][];
  reference_hardware: string;
  paths: AppPaths;
  prompt_versions: [string, string][];
}

export interface HistoryRow {
  id: number;
  asked_at: string;
  question: string;
  canon_mode: string;
  model_id: string;
  index_version: string;
  crisis_flag: boolean;
  verdict: string;
  fallback_used: boolean;
  preview: string;
  cited_count: number;
}

export interface Retrieved {
  passages: PassageOut[];
  topic_groups: TopicGroup[];
}

export interface HistoryDetail {
  row: HistoryRow;
  answer_md: string;
  timings: Timings;
  passages: PassageOut[];
  tokens_resolvable: boolean;
  index_note: string | null;
}
