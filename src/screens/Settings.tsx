// Settings and About, per PLAN 7.3 as amended.
//
// Every setting is written to user.db by a command. Nothing is kept in the
// window, so closing and reopening the app finds exactly what the reader left.

import { useEffect, useState } from "react";

import { save } from "@tauri-apps/plugin-dialog";

import * as api from "../api";
import { Progress } from "./FirstRun";
import type { AppInfo, AppSettings, DownloadProgress, ModelStatus } from "../types";

interface Props {
  info: AppInfo;
  settings: AppSettings;
  models: ModelStatus[];
  historyCount: number;
  onChange: (s: AppSettings) => void;
  onClose: () => void;
  onHistoryChanged: () => void;
}

export function SettingsScreen({
  info,
  settings,
  models,
  historyCount,
  onChange,
  onClose,
  onHistoryChanged,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);

  useEffect(() => {
    let un: (() => void) | null = null;
    api.onDownloadProgress(setProgress).then((u) => (un = u));
    return () => un?.();
  }, []);

  async function set(key: string, value: string) {
    setError(null);
    try {
      onChange(await api.setSetting(key, value));
    } catch (e) {
      setError(String(e));
    }
  }

  async function chooseModel(id: string) {
    const m = models.find((x) => x.id === id);
    if (m && !m.present) {
      setBusy(true);
      setMessage(`Downloading ${m.label.toLowerCase()}…`);
      try {
        await api.downloadModel(id);
        setMessage(null);
      } catch (e) {
        setError(String(e));
        setBusy(false);
        return;
      }
      setBusy(false);
    }
    await set("model", id);
    setMessage("The model will be loaded on your next question.");
  }

  async function exportHistory() {
    setError(null);
    try {
      const path = await save({
        title: "Save question history",
        defaultPath: "pastor-bible-history.txt",
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (!path) return;
      await api.historyExport(path);
      setMessage(`Saved to ${path}`);
    } catch (e) {
      setError(String(e));
    }
  }

  async function clearHistory() {
    setError(null);
    try {
      const n = await api.historyClear();
      setConfirmClear(false);
      setMessage(`${n} question${n === 1 ? "" : "s"} deleted.`);
      onHistoryChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  const chatModels = models.filter((m) => !m.bundled);

  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal stack" onClick={(e) => e.stopPropagation()}>
        <div className="row between">
          <h1 style={{ margin: 0 }}>Settings</h1>
          <button className="quiet" onClick={onClose}>
            Close
          </button>
        </div>

        {error && <div className="err">{error}</div>}
        {message && <div className="notice good">{message}</div>}

        <div className="field">
          <label>Which books</label>
          <div className="faint">
            The 66 books are always included. The Deuterocanon is what some traditions include and
            others do not; every passage from it is labelled.
          </div>
          <div className="choices">
            <button
              className={settings.canon === "66" ? "choice on" : "choice"}
              onClick={() => set("canon", "66")}
            >
              66 books
            </button>
            <button
              className={settings.canon === "both" ? "choice on" : "choice"}
              onClick={() => set("canon", "both")}
            >
              Include the Deuterocanon
            </button>
          </div>
        </div>

        <div className="field">
          <label>Answering model</label>
          <div className="choices">
            {chatModels.map((m) => (
              <button
                key={m.id}
                className={settings.model === m.id ? "choice on" : "choice"}
                onClick={() => chooseModel(m.id)}
                disabled={busy}
              >
                {m.label}
                {!m.present && <span className="faint"> · {api.bytes(m.bytes)} to download</span>}
              </button>
            ))}
          </div>
          <div className="faint" style={{ marginTop: 6 }}>
            {chatModels.find((m) => m.id === settings.model)?.note}
          </div>
          {busy && <Progress p={progress} />}
        </div>

        <div className="field">
          <label>Compute</label>
          <div className="faint">
            The Pastor Bible answers on the processor today. Using the graphics card is much
            faster and arrives in a later release.
          </div>
          <div className="choices">
            <button
              className={settings.compute === "auto" ? "choice on" : "choice"}
              onClick={() => set("compute", "auto")}
            >
              Auto
            </button>
            <button
              className={settings.compute === "cpu" ? "choice on" : "choice"}
              onClick={() => set("compute", "cpu")}
            >
              Processor
            </button>
            <button className="choice" disabled title="available in a later release">
              Graphics card · available in a later release
            </button>
          </div>
        </div>

        <div className="field">
          <label>Question history</label>
          <div className="faint">
            {historyCount} question{historyCount === 1 ? "" : "s"} kept on this computer, in{" "}
            {info.paths.user_db}. Nothing here has ever been sent anywhere.
          </div>
          <div className="choices">
            <button onClick={exportHistory}>Export to a text file</button>
            {confirmClear ? (
              <>
                <button className="danger" onClick={clearHistory}>
                  Yes, delete all {historyCount}
                </button>
                <button className="quiet" onClick={() => setConfirmClear(false)}>
                  Keep them
                </button>
              </>
            ) : (
              <button className="danger" onClick={() => setConfirmClear(true)} disabled={historyCount === 0}>
                Delete all history
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export function AboutScreen({ info, onClose }: { info: AppInfo; onClose: () => void }) {
  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal stack" onClick={(e) => e.stopPropagation()}>
        <div className="row between">
          <h1 style={{ margin: 0 }}>About</h1>
          <button className="quiet" onClick={onClose}>
            Close
          </button>
        </div>

        <p className="muted">
          The Pastor Bible is a free, offline Bible study tool. It is nondenominational: it uses one
          public-domain translation, reports what the text says and where, includes no commentary,
          and takes no position where Christian traditions differ.
        </p>

        <blockquote className="statement">{info.disclaimer}</blockquote>
        <blockquote className="statement">{info.crisis_note}</blockquote>

        <div className="field">
          <label>Privacy</label>
          <div>{info.offline_statement}</div>
        </div>

        <table className="spec">
          <tbody>
            <tr>
              <td>Version</td>
              <td>{info.app_version}</td>
            </tr>
            <tr>
              <td>Bible index</td>
              <td>{info.index_version}</td>
            </tr>
            <tr>
              <td>Answering model</td>
              <td>{info.model_file}</td>
            </tr>
            <tr>
              <td>Search model</td>
              <td>{info.embedding_model}</td>
            </tr>
            <tr>
              <td>Built and tested on</td>
              <td>{info.reference_hardware}</td>
            </tr>
            <tr>
              <td>Your files</td>
              <td>{info.paths.app_data}</td>
            </tr>
          </tbody>
        </table>

        <div className="field">
          <label>Made by</label>
          <div>{info.authors.join(" and ")}</div>
          <div className="faint">Licensed {info.license}.</div>
        </div>

        <div className="field">
          <label>Sources</label>
          <table className="spec">
            <tbody>
              {info.sources.map(([name, license]) => (
                <tr key={name}>
                  <td>{name}</td>
                  <td className="ref">{license}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
