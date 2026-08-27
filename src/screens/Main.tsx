// The main window.
//
// Three things here are the plan's, not preferences. The passages appear the
// moment retrieval returns, which is under a tenth of a second, so the reader is
// in the text while the answer is still being written. The synopsis appears
// only once the verifier has passed it, so nothing unverified is ever on
// screen, even briefly. And the crisis note, when it appears, is above the
// answer and never instead of it.

import { useCallback, useEffect, useRef, useState } from "react";

import * as api from "../api";
import { PassagePanel } from "../components/PassagePanel";
import { Synopsis } from "../components/Synopsis";
import { StageLine, stageText } from "./FirstRun";
import type {
  Answer,
  AppInfo,
  AppSettings,
  HistoryDetail,
  HistoryRow,
  PassageOut,
  Stage,
} from "../types";

interface Props {
  info: AppInfo;
  settings: AppSettings;
  onSettingsChange: (s: AppSettings) => void;
  onOpenSettings: () => void;
  onOpenAbout: () => void;
}

export function Main({ info, settings, onSettingsChange, onOpenSettings, onOpenAbout }: Props) {
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState<Answer | null>(null);
  const [past, setPast] = useState<HistoryDetail | null>(null);
  const [stage, setStage] = useState<Stage | null>(null);
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [crisisNote, setCrisisNote] = useState<string | null>(null);
  const [highlight, setHighlight] = useState<string | null>(null);

  const [history, setHistory] = useState<HistoryRow[]>([]);
  const [search, setSearch] = useState("");
  const [activeId, setActiveId] = useState<number | null>(null);

  const unlisten = useRef<null | (() => void)>(null);
  const timer = useRef<number | null>(null);

  const refreshHistory = useCallback(async () => {
    try {
      setHistory(search.trim() ? await api.historySearch(search) : await api.historyList());
    } catch {
      /* the sidebar is not worth an error banner */
    }
  }, [search]);

  useEffect(() => {
    api.onStage(setStage).then((u) => (unlisten.current = u));
    return () => unlisten.current?.();
  }, []);

  useEffect(() => {
    void refreshHistory();
  }, [refreshHistory]);

  useEffect(() => {
    if (!running) {
      if (timer.current) window.clearInterval(timer.current);
      return;
    }
    const started = Date.now();
    setElapsed(0);
    timer.current = window.setInterval(() => setElapsed((Date.now() - started) / 1000), 250);
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
  }, [running]);

  // The passages go on screen the moment retrieval returns, before the answer
  // exists. Until then there is nothing to show but the stage line.
  const [earlyPassages, setEarlyPassages] = useState<PassageOut[] | null>(null);
  useEffect(() => {
    if (stage?.stage === "retrieving") setEarlyPassages(null);
  }, [stage]);

  async function onAsk() {
    const q = question.trim();
    if (!q || running) return;
    setRunning(true);
    setError(null);
    setAnswer(null);
    setPast(null);
    setActiveId(null);
    setEarlyPassages(null);
    setCrisisNote(null);
    try {
      const a = await api.ask(q);
      setAnswer(a);
      if (a.crisis) setCrisisNote(a.crisis_note ?? info.crisis_note);
      void refreshHistory();
    } catch (e) {
      const msg = String(e);
      setError(msg.includes("cancelled") ? null : msg);
    } finally {
      setRunning(false);
    }
  }

  async function openEntry(id: number) {
    setError(null);
    setAnswer(null);
    setActiveId(id);
    try {
      const d = await api.historyGet(id);
      setPast(d);
      setCrisisNote(d?.row.crisis_flag ? info.crisis_note : null);
      setQuestion(d?.row.question ?? "");
    } catch (e) {
      setError(String(e));
    }
  }

  const shownPassages = answer?.passages ?? past?.passages ?? earlyPassages ?? [];
  const shownMarkdown = answer
    ? answer.synopsis_markdown ?? answer.fallback_markdown
    : past?.answer_md ?? null;
  const isFallback = answer ? answer.fallback_used : false;

  return (
    <div className="app">
      <aside className="sidebar">
        <header className="stack" style={{ ["--gap" as string]: "10px" }}>
          <div className="title">The Pastor Bible</div>
          <input
            type="search"
            placeholder="Search past questions"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </header>
        <div className="list">
          {history.length === 0 && (
            <p className="faint" style={{ padding: "10px 12px" }}>
              {search.trim() ? "Nothing matches." : "Your questions will be listed here."}
            </p>
          )}
          {history.map((h) => (
            <button
              key={h.id}
              className={activeId === h.id ? "entry active" : "entry"}
              onClick={() => openEntry(h.id)}
            >
              <div className="q">{h.question}</div>
              <div className="meta">
                {api.whenAsked(h.asked_at)}
                {h.canon_mode === "both" && " · with Deuterocanon"}
                {h.fallback_used && " · passages only"}
              </div>
            </button>
          ))}
        </div>
        <footer className="row between">
          <button className="quiet" onClick={onOpenSettings}>
            Settings
          </button>
          <button className="quiet" onClick={onOpenAbout}>
            About
          </button>
        </footer>
      </aside>

      <main className="main">
        <div className="inner stack">
          <div className="asker stack" style={{ ["--gap" as string]: "10px" }}>
            <textarea
              placeholder="What does the Bible say about…"
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void onAsk();
              }}
            />
            <div className="row between">
              <div className="row">
                <button
                  className={settings.canon === "66" ? "choice on" : "choice"}
                  onClick={() => api.setSetting("canon", "66").then(onSettingsChange)}
                  disabled={running}
                >
                  66 books
                </button>
                <button
                  className={settings.canon === "both" ? "choice on" : "choice"}
                  onClick={() => api.setSetting("canon", "both").then(onSettingsChange)}
                  disabled={running}
                >
                  Include Deuterocanon
                </button>
              </div>
              <div className="row">
                {running && <button onClick={() => api.cancelAsk()}>Stop</button>}
                <button className="primary" onClick={onAsk} disabled={running || !question.trim()}>
                  {running ? "Working…" : "Ask"}
                </button>
              </div>
            </div>
            <div className="faint">Ctrl+Enter asks. Answers take a few minutes on this machine.</div>
          </div>

          {running && <StageLine stage={stage} elapsed={elapsed} />}
          {!running && stage?.stage === "cancelled" && !answer && (
            <div className="notice">Stopped. Nothing was saved.</div>
          )}
          {error && <div className="err">{error}</div>}

          {crisisNote && (
            <div className="notice crisis">
              <strong>If you are in crisis</strong>
              <p style={{ margin: "6px 0 0" }}>{crisisNote}</p>
            </div>
          )}

          {past?.index_note && <div className="notice">{past.index_note}</div>}

          {shownMarkdown && (
            <div>
              {isFallback && (
                <div className="notice" style={{ marginBottom: 14 }}>
                  A synthesis could not be produced for this question, so the passages that were
                  found are listed instead. Nothing here is invented.
                </div>
              )}
              {isFallback ? (
                <pre style={{ whiteSpace: "pre-wrap", fontFamily: "var(--sans)" }}>
                  {shownMarkdown}
                </pre>
              ) : (
                <Synopsis
                  markdown={shownMarkdown}
                  passages={shownPassages}
                  onSelect={(ref) => {
                    setHighlight(ref);
                    document
                      .getElementById(`passage-${ref.replace(/[^A-Za-z0-9]/g, "-")}`)
                      ?.scrollIntoView({ behavior: "smooth", block: "center" });
                  }}
                />
              )}
              {answer?.deuterocanon_footer && (
                <p className="faint" style={{ marginTop: 18 }}>
                  {answer.deuterocanon_footer}
                </p>
              )}
              {answer && (
                <p className="faint" style={{ marginTop: 10 }}>
                  {answer.model_id} · Bible index {answer.index_version} ·{" "}
                  {api.duration(answer.timings.total_seconds)}
                  {answer.attempts.length > 1 && " · written twice, the first broke the citation rule"}
                </p>
              )}
            </div>
          )}

          {shownPassages.length > 0 && (
            <PassagePanel
              passages={shownPassages}
              groups={answer?.topic_groups ?? []}
              groupBy={
                (answer?.topic_groups?.length ?? 0) > 0
                  ? (settings.group_by as "topic" | "book")
                  : "book"
              }
              onGroupByChange={(v) => api.setSetting("group_by", v).then(onSettingsChange)}
              highlight={highlight}
            />
          )}

          {!shownMarkdown && !running && shownPassages.length === 0 && (
            <div className="stack" style={{ marginTop: 40 }}>
              <h2>Ask anything about the Bible</h2>
              <p className="muted">
                The Pastor Bible searches the whole text, shows you every passage it found, and then
                writes a short synopsis of what those passages say. It cites only passages that were
                actually found: it cannot invent a reference, and a reference that does not check out
                is never shown to you.
              </p>
              <p className="faint">{info.offline_statement}</p>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}

export { stageText };
