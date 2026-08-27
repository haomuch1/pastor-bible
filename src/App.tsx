// What the window shows: first run once, then the main screen.

import { useCallback, useEffect, useState } from "react";

import * as api from "./api";
import { FirstRun } from "./screens/FirstRun";
import { Main } from "./screens/Main";
import { AboutScreen, SettingsScreen } from "./screens/Settings";
import type { AppInfo, AppSettings, StartupState } from "./types";

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [startup, setStartup] = useState<StartupState | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [firstRunDone, setFirstRunDone] = useState(false);

  const load = useCallback(async () => {
    try {
      const [i, s] = await Promise.all([api.appInfo(), api.startupState()]);
      setInfo(i);
      setStartup(s);
      setSettings(s.settings);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // The screen is a preview target during development, so a hard reload must
  // not leave two sidecars running. The window-close handler in Rust does the
  // real work; this asks politely first.
  useEffect(() => {
    const bye = () => {
      void api.shutdownModels();
    };
    window.addEventListener("beforeunload", bye);
    return () => window.removeEventListener("beforeunload", bye);
  }, []);

  if (error) {
    return (
      <div className="first-run stack">
        <h1>The Pastor Bible could not start</h1>
        <div className="err">{error}</div>
        <p className="muted">
          This usually means the Bible index or the model server is not where the program expects
          it. Reinstalling puts both back.
        </p>
      </div>
    );
  }

  if (!info || !startup || !settings) {
    return (
      <div className="first-run">
        <p className="faint">Starting…</p>
      </div>
    );
  }

  const needsFirstRun = startup.first_run && !firstRunDone;
  if (needsFirstRun) {
    return (
      <FirstRun
        info={info}
        startup={startup}
        onDone={async () => {
          await api.finishFirstRun();
          setFirstRunDone(true);
          void load();
        }}
      />
    );
  }

  return (
    <>
      <Main
        info={info}
        settings={settings}
        onSettingsChange={setSettings}
        onOpenSettings={() => setShowSettings(true)}
        onOpenAbout={() => setShowAbout(true)}
        modelProblem={startup.model_problem}
        onHistoryChanged={load}
      />
      {showSettings && (
        <SettingsScreen
          info={info}
          settings={settings}
          models={startup.models}
          historyCount={startup.history_count}
          onChange={setSettings}
          onClose={() => setShowSettings(false)}
          onHistoryChanged={load}
        />
      )}
      {showAbout && <AboutScreen info={info} onClose={() => setShowAbout(false)} />}
    </>
  );
}
