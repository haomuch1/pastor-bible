// PLAN 7.1, as amended on 2026-08-26.
//
// Welcome, an advisory hardware check, the one download, the self-test, then
// the main screen. The hardware check never refuses: it shows this machine
// beside the machine the app was tested on, says one plain sentence if
// something is below it, and Continue is always enabled.

import { useEffect, useRef, useState } from "react";

import * as api from "../api";
import type { AppInfo, DownloadProgress, Hardware, SelfTestResult, StartupState, Stage } from "../types";

type Step = "welcome" | "hardware" | "download" | "selftest";

interface Props {
  info: AppInfo;
  startup: StartupState;
  onDone: () => void;
}

const STEPS: [Step, string][] = [
  ["welcome", "Welcome"],
  ["hardware", "This computer"],
  ["download", "One download"],
  ["selftest", "A quick check"],
];

export function FirstRun({ info, startup, onDone }: Props) {
  const [step, setStep] = useState<Step>("welcome");

  return (
    <div className="first-run stack">
      <div className="steps">
        {STEPS.map(([id, label], i) => {
          const at = STEPS.findIndex(([s]) => s === step);
          const cls = i === at ? "now" : i < at ? "done" : "";
          return (
            <span key={id} className={cls}>
              {i + 1}. {label}
            </span>
          );
        })}
      </div>

      {step === "welcome" && <Welcome info={info} onNext={() => setStep("hardware")} />}
      {step === "hardware" && <HardwareStep onNext={() => setStep("download")} />}
      {step === "download" && (
        <DownloadStep startup={startup} info={info} onNext={() => setStep("selftest")} />
      )}
      {step === "selftest" && <SelfTestStep onDone={onDone} />}
    </div>
  );
}

// ---------------------------------------------------------------- welcome

function Welcome({ info, onNext }: { info: AppInfo; onNext: () => void }) {
  return (
    <div className="stack">
      <h1>The Pastor Bible</h1>
      <p className="muted">
        A free, offline Bible study tool. Ask a question and it shows you what the text says,
        with every passage it found.
      </p>

      <h2>Please read this first</h2>
      <blockquote className="statement">{info.disclaimer}</blockquote>

      <h2>If you are in crisis</h2>
      <blockquote className="statement">{info.crisis_note}</blockquote>

      <p className="faint">
        Made by {info.authors.join(" and ")}. {info.license} licensed. {info.offline_statement}
      </p>

      <div className="row end">
        <button className="primary" onClick={onNext}>
          Continue
        </button>
      </div>
    </div>
  );
}

// --------------------------------------------------------------- hardware

function HardwareStep({ onNext }: { onNext: () => void }) {
  const [hw, setHw] = useState<Hardware | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.hardwareCheck().then(setHw).catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="stack">
      <h1>This computer</h1>
      <p className="muted">
        The Pastor Bible is built and tested on one machine. Here is yours beside it. Whatever it
        says, you can carry on: the app does not refuse to run.
      </p>

      {error && <div className="err">{error}</div>}
      {!hw && !error && <p className="faint">Looking…</p>}

      {hw && (
        <>
          <table className="spec">
            <tbody>
              <tr>
                <td>Processor</td>
                <td>
                  {hw.cpu}
                  {hw.cores > 0 && <span className="faint"> · {hw.cores} threads</span>}
                </td>
                <td className="ref">tested on {hw.reference.cpu}</td>
              </tr>
              <tr>
                <td>Memory</td>
                <td>{hw.ram_gb.toFixed(0)} GB</td>
                <td className="ref">tested on {hw.reference.ram_gb.toFixed(0)} GB</td>
              </tr>
              <tr>
                <td>Free disk</td>
                <td>
                  {hw.free_disk_gb.toFixed(0)} GB free
                  {hw.disk_drive && <span className="faint"> on {hw.disk_drive}</span>}
                </td>
                <td className="ref">about {hw.reference.disk_gb.toFixed(0)} GB needed</td>
              </tr>
              {/* The devices the model server can see, which is the same list
                  Settings > Compute shows. This row used to name one display
                  adapter from the OS and say "not used yet", on a machine with
                  an RTX 3050 and an app that has used graphics cards since P6. */}
              <tr>
                <td>Graphics</td>
                <td>
                  {hw.gpu_devices.length === 0
                    ? "None the model server can use"
                    : hw.gpu_devices.map((d) => (
                        <div key={d.name}>
                          {d.name}
                          <span className="faint">
                            {" "}
                            · {(d.total_mib / 1024).toFixed(1)} GB, {(d.free_mib / 1024).toFixed(1)} GB free
                          </span>
                        </div>
                      ))}
                </td>
                <td className="ref">tested on {hw.reference.gpu}</td>
              </tr>
              <tr>
                <td>System</td>
                <td>{hw.os}</td>
                <td className="ref">tested on {hw.reference.os}</td>
              </tr>
            </tbody>
          </table>

          {hw.graphics && <p className="muted">{hw.graphics}</p>}

          {hw.warning ? (
            <div className="notice">
              <strong>{hw.warning}</strong>
              <ul style={{ margin: "8px 0 0", paddingLeft: "1.2em" }}>
                {hw.below.map((b, i) => (
                  <li key={i}>{b}</li>
                ))}
              </ul>
            </div>
          ) : (
            <div className="notice good">
              This computer matches the machine The Pastor Bible was tested on.
            </div>
          )}

          <div className="row end">
            <button className="primary" onClick={onNext}>
              Continue
            </button>
          </div>
        </>
      )}
    </div>
  );
}

// --------------------------------------------------------------- download

function DownloadStep({
  startup,
  info,
  onNext,
}: {
  startup: StartupState;
  info: AppInfo;
  onNext: () => void;
}) {
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [running, setRunning] = useState(false);
  const [done, setDone] = useState(startup.chat_model_present);
  const [error, setError] = useState<string | null>(null);
  const unlisten = useRef<null | (() => void)>(null);

  useEffect(() => {
    api.onDownloadProgress(setProgress).then((u) => (unlisten.current = u));
    return () => unlisten.current?.();
  }, []);

  const model = startup.models.find((m) => m.id === startup.settings.model);
  const resumable = (model?.partial_bytes ?? 0) > 0;

  async function start() {
    setRunning(true);
    setError(null);
    try {
      await api.downloadModel(startup.settings.model);
      setDone(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="stack">
      <h1>One download</h1>
      <p className="statement" style={{ padding: "14px 18px" }}>
        This is the only time The Pastor Bible needs the internet.
      </p>
      <p className="muted">
        The answering model is {info.model_file}, about {api.bytes(model?.bytes ?? 0)}. Everything
        else, including the Bible itself, came with the installer. If the download is interrupted
        it carries on from where it stopped.
      </p>

      {/* On an Intel Mac the model server has no graphics path at all -- the
          x64 llama.cpp build carries no Metal backend -- so the reader is told
          what that means for them before they commit to a 4.7 GB download,
          not after. Null on every other build. */}
      {info.no_gpu_platform_note && (
        <div className="notice">{info.no_gpu_platform_note}</div>
      )}

      {done && (
        <div className="notice good">
          The answering model is here and its checksum matches. Nothing needs downloading.
        </div>
      )}

      {!done && (
        <>
          {resumable && !running && (
            <p className="faint">
              {api.bytes(model!.partial_bytes)} of {api.bytes(model!.bytes)} was fetched before.
              It will carry on from there.
            </p>
          )}
          <Progress p={progress} />
          {error && <div className="err">{error}</div>}
          <div className="row">
            <button className="primary" onClick={start} disabled={running}>
              {running ? "Downloading…" : resumable ? "Carry on downloading" : "Download"}
            </button>
            {running && (
              <button onClick={() => api.cancelDownload()}>Stop</button>
            )}
          </div>
        </>
      )}

      <div className="row end">
        <button className="primary" onClick={onNext} disabled={!done}>
          Continue
        </button>
      </div>
    </div>
  );
}

export function Progress({ p }: { p: DownloadProgress | null }) {
  if (!p) return null;
  if (p.stage === "downloading") {
    return (
      <div className="stack" style={{ ["--gap" as string]: "8px" }}>
        <div className="bar">
          <div style={{ width: `${p.percent.toFixed(1)}%` }} />
        </div>
        <div className="faint">
          {api.bytes(p.done)} of {api.bytes(p.total)} · {p.percent.toFixed(1)}% ·{" "}
          {api.bytes(Math.round(p.bytes_per_second))}/s
          {p.eta_seconds != null && ` · about ${api.duration(p.eta_seconds)} left`}
          {p.resumed_from > 0 && ` · carried on from ${api.bytes(p.resumed_from)}`}
        </div>
      </div>
    );
  }
  if (p.stage === "verifying") {
    return (
      <div className="stack" style={{ ["--gap" as string]: "8px" }}>
        <div className="bar">
          <div style={{ width: `${p.percent.toFixed(1)}%` }} />
        </div>
        <div className="faint">Checking the file is exactly the right one… {p.percent.toFixed(0)}%</div>
      </div>
    );
  }
  if (p.stage === "checking") return <div className="faint">Checking {p.file}…</div>;
  if (p.stage === "done")
    return (
      <div className="notice good">
        {p.skipped ? "Already here and correct." : `Downloaded and verified, ${api.bytes(p.bytes)}.`}
      </div>
    );
  return <div className="err">{p.message}</div>;
}

// --------------------------------------------------------------- selftest

function SelfTestStep({ onDone }: { onDone: () => void }) {
  const [stage, setStage] = useState<Stage | null>(null);
  const [result, setResult] = useState<SelfTestResult | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const unlisten = useRef<null | (() => void)>(null);

  useEffect(() => {
    api.onStage(setStage).then((u) => (unlisten.current = u));
    return () => unlisten.current?.();
  }, []);

  async function run() {
    setRunning(true);
    setError(null);
    try {
      setResult(await api.runSelfTest());
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="stack">
      <h1>A quick check</h1>
      <p className="muted">
        Three questions are asked and answered, to be sure everything works before you start. On
        this kind of machine it takes a few minutes. Nothing leaves this computer.
      </p>

      {!result && (
        <div className="row">
          <button className="primary" onClick={run} disabled={running}>
            {running ? "Working…" : "Run the check"}
          </button>
          {running && <StageLine stage={stage} />}
        </div>
      )}

      {error && <div className="err">{error}</div>}

      {result && (
        <>
          <div className={result.passed ? "notice good" : "notice"}>
            {result.passed
              ? `All three questions were answered and every reference checked out. ${api.duration(result.seconds)}.`
              : "Something did not check out. The details are below."}
          </div>
          <table className="spec">
            <tbody>
              {result.questions.map((q) => (
                <tr key={q.id}>
                  <td>{q.ok ? "✓" : "✕"}</td>
                  <td>
                    {q.question}
                    <div className="faint">
                      {q.cited} of {q.sent} passages used · {q.fabrications} invented references ·{" "}
                      {api.duration(q.seconds)}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="row end">
            <button className="primary" onClick={onDone}>
              Start using The Pastor Bible
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export function StageLine({ stage, elapsed }: { stage: Stage | null; elapsed?: number }) {
  if (!stage) return null;
  return (
    <div className="stageline">
      <span className="dot" />
      <span className="grow">{stageText(stage)}</span>
      {elapsed != null && <span className="faint">{api.clock(elapsed)}</span>}
    </div>
  );
}

export function stageText(s: Stage): string {
  switch (s.stage) {
    case "loading_model":
      return `Loading the ${s.role} model…`;
    case "retrieving":
      return "Searching the text…";
    case "retrieved":
      return `Found ${s.passages} passages; reading ${s.sent} of them.`;
    case "generating":
      return s.attempt > 1
        ? `Writing the answer again… ${s.tokens} words so far`
        : `Writing the answer… ${s.tokens} words so far`;
    case "checking_references":
      return "Checking every reference against the passages found…";
    case "retrying":
      return `${s.reason} Writing it again.`;
    case "done":
      return "Done.";
    case "cancelled":
      return "Stopped.";
    case "failed":
      return s.message;
  }
}
