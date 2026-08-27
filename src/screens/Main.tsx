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
import { Reader } from "../components/Reader";
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
  /// A model file that is missing or is not the one we pinned, said plainly.
  /// Shown here rather than only at the moment a question fails, so the reader
  /// learns about it before the wait rather than after it.
  modelProblem?: string | null;
  /// The history changed under the sidebar, so whatever else counts entries
  /// should count them again.
  onHistoryChanged?: () => void;
}

export function Main({
  info,
  settings,
  onSettingsChange,
  onOpenSettings,
  onOpenAbout,
  modelProblem,
  onHistoryChanged,
}: Props) {
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
  // The entry whose delete has been pressed once. Null when none is asking.
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);

  // Which processor will answer. Probed once by the backend and read here so
  // the reader is told what they are waiting for before they commit to it.
  const [compute, setCompute] = useState<string | null>(null);
  useEffect(() => {
    api
      .computeStatus()
      .then((c) => setCompute(c.using))
      .catch(() => setCompute(null));
  }, [settings.compute, settings.model]);

  // The chapter a passage came from, when the reader has asked to read one.
  // The answer underneath is never unmounted, so closing this puts them back
  // exactly where they were.
  const [reading, setReading] = useState<{
    bookId: number;
    chapter: number;
    highlight: number[];
  } | null>(null);

  const box = useRef<HTMLTextAreaElement | null>(null);

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

  // The passages go on screen the moment retrieval returns, about forty
  // milliseconds in, so the reader is in the text for the whole of the two and
  // a half minutes the answer takes to write.
  const [earlyPassages, setEarlyPassages] = useState<PassageOut[] | null>(null);
  useEffect(() => {
    if (stage?.stage === "retrieving") {
      setEarlyPassages(null);
    }
    if (stage?.stage === "retrieved") {
      // Fetched rather than pushed: a quarter of a megabyte does not survive
      // the event channel, and this is the channel the answer itself uses.
      void api
        .retrievedPassages()
        .then((r) => {
          if (r) {
            setEarlyPassages(r.passages);
          }
        })
        .catch(() => {
          /* the answer will bring them in a couple of minutes either way */
        });
    }
  }, [stage]);

  /// Back to the empty screen, with the question box ready.
  ///
  /// Always reachable, from the top of the main area and from the sidebar, so
  /// that opening a past answer is never a door that closes behind the reader.
  function newQuestion() {
    setQuestion("");
    setAnswer(null);
    setPast(null);
    setActiveId(null);
    setEarlyPassages(null);
    setCrisisNote(null);
    setError(null);
    setStage(null);
    setHighlight(null);
    setReading(null);
    window.setTimeout(() => box.current?.focus(), 0);
  }

  /// Open a passage in its chapter. The verse ids are what gets marked.
  function readPassage(p: PassageOut) {
    const first = p.verse_ids[0];
    if (first == null) return;
    setReading({
      bookId: Math.floor(first / 1_000_000),
      chapter: Math.floor((first % 1_000_000) / 1000),
      highlight: p.verse_ids,
    });
  }

  /// Delete one entry, and only that one.
  ///
  /// Never navigates: an entry the reader is deleting is not an entry they are
  /// asking to read. If it happens to be the one on screen, the screen goes
  /// back to empty rather than showing an answer that no longer exists.
  async function deleteEntry(id: number) {
    setConfirmDelete(null);
    try {
      await api.historyDelete(id);
      if (activeId === id) newQuestion();
      void refreshHistory();
      onHistoryChanged?.();
    } catch (e) {
      setError(String(e));
    }
  }

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
  const unlinked = past != null && !past.tokens_resolvable;
  const shownMarkdown = answer
    ? answer.synopsis_markdown ?? answer.fallback_markdown
    : past
      ? // An answer stored before the citation markers were kept: the markers
        // cannot be linked to passages, so they are removed rather than shown
        // pointing at the wrong ones.
        unlinked
        ? past.answer_md.replace(/\s*\[P\d+\]/g, "")
        : past.answer_md
      : null;
  const isFallback = answer ? answer.fallback_used : false;

  return (
    <div className="app">
      <aside className="sidebar">
        <header className="stack" style={{ ["--gap" as string]: "10px" }}>
          <div className="title">The Pastor Bible</div>
          <button className="quiet wide" onClick={newQuestion} disabled={running}>
            New question
          </button>
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
            // A row rather than one button: the delete sits beside the entry,
            // not inside it, so pressing it cannot also open the answer.
            <div key={h.id} className={activeId === h.id ? "entry active" : "entry"}>
              <button className="entry-open" onClick={() => openEntry(h.id)}>
                <div className="q">{h.question}</div>
                <div className="meta">
                  {api.whenAsked(h.asked_at)}
                  {h.canon_mode === "both" && " · with Deuterocanon"}
                  {h.fallback_used && " · passages only"}
                </div>
              </button>
              {confirmDelete === h.id ? (
                <div className="entry-confirm">
                  <button className="danger tiny" onClick={() => void deleteEntry(h.id)}>
                    Delete
                  </button>
                  <button className="quiet tiny" onClick={() => setConfirmDelete(null)}>
                    Cancel
                  </button>
                </div>
              ) : (
                <button
                  className="entry-delete"
                  title="Delete this question"
                  aria-label="Delete this question"
                  onClick={() => setConfirmDelete(h.id)}
                >
                  <TrashIcon />
                </button>
              )}
            </div>
          ))}
        </div>
        {/* Deleting one entry is done on the entry. Deleting all of them is a
            different kind of act and lives in Settings, where it is next to
            the export that would have saved them first. */}
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
          <div className="row between">
            <button className="quiet" onClick={newQuestion} disabled={running}>
              New question
            </button>
            {past && <span className="faint">A past answer, reopened</span>}
          </div>

          {modelProblem && <div className="err prose">{modelProblem}</div>}

          <div className="asker stack" style={{ ["--gap" as string]: "10px" }}>
            <textarea
              ref={box}
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
            {/* What the reader is about to wait for, from the path that will
                actually run rather than from what Settings asks for: Auto
                resolves to one or the other at the moment a model is loaded. */}
            <div className="faint">Ctrl+Enter asks. {answerTimeHint(compute)}</div>
          </div>

          {running && <StageLine stage={stage} elapsed={elapsed} />}
          {!running && stage?.stage === "cancelled" && !answer && (
            <div className="notice">Stopped. Nothing was saved.</div>
          )}
          {/* A refusal that is a paragraph keeps its paragraphs: the message
              naming a missing model file is the same message either way. */}
          {error && <div className="err prose">{error}</div>}

          {crisisNote && (
            <div className="notice crisis">
              <strong>If you are in crisis</strong>
              <p style={{ margin: "6px 0 0" }}>{crisisNote}</p>
            </div>
          )}

          {past?.index_note && <div className="notice">{past.index_note}</div>}
          {unlinked && (
            <div className="notice">
              This answer was saved before The Pastor Bible kept track of which passage each
              citation pointed at. The passages it rested on are below; the markers in the text
              have been removed rather than shown pointing at the wrong ones.
            </div>
          )}

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

          {running && earlyPassages && earlyPassages.length > 0 && !answer && (
            <p className="faint">
              These are the passages the search found. Read them while the answer is written; the
              answer appears only once every reference in it has been checked.
            </p>
          )}

          {shownPassages.length > 0 && (
            <PassagePanel passages={shownPassages} highlight={highlight} onRead={readPassage} />
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

      {reading && (
        <Reader
          bookId={reading.bookId}
          chapter={reading.chapter}
          highlight={reading.highlight}
          onClose={() => setReading(null)}
        />
      )}
    </div>
  );
}

/// A waste basket, drawn rather than fetched: this app loads no asset it did
/// not ship, and one icon is not a reason to start.
function TrashIcon() {
  return (
    <svg
      width="15"
      height="15"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M2.5 4h11" />
      <path d="M6.5 4V2.5h3V4" />
      <path d="M4 4l.6 9a1 1 0 0 0 1 .9h4.8a1 1 0 0 0 1-.9L12 4" />
      <path d="M6.6 6.8v4.6M9.4 6.8v4.6" />
    </svg>
  );
}

/// How long an answer takes, said before the reader commits to waiting.
///
/// P4 measured the same question at 12 seconds on the GPU sidecar and 178 on
/// the CPU one. That is not a detail to leave the reader to discover.
export function answerTimeHint(compute: string | null | undefined): string {
  if (compute === "gpu") return "Answers usually take under a minute on this machine.";
  if (compute === "cpu") return "Answers take a few minutes on this machine.";
  // Auto has not resolved yet: no model has been loaded this session.
  return "Answers take a few minutes on this machine.";
}

export { stageText };
