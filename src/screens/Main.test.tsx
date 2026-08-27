// The history sidebar, rendered.
//
// This file exists because of a specific failure. P5 reported that the sidebar
// had a per-entry delete; P5.1 repeated the claim in its handoff; Jared then
// hovered and clicked over every entry in the running app and found nothing.
// The control had never been written. Everything underneath it had:
// `UserDb::delete`, the `history_delete` command, the `historyDelete` binding
// in api.ts, and a passing cargo test called
// `deleting_removes_exactly_one_and_search_forgets_it`. Only the button was
// missing, and no test could see that, because nothing rendered the component.
//
// So these tests render it. They cannot see pixels — jsdom does no layout — but
// they can say that the control is there, that a reader can find it by the name
// it announces, and that pressing it deletes one entry and no others and does
// not open anything.

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../api";
import { Main } from "./Main";
import type { AppInfo, AppSettings, HistoryRow } from "../types";

const rows: HistoryRow[] = [
  row(1, "What does the Bible say about anxiety?"),
  row(2, "What does the Bible say about tithing while unemployed?"),
  row(3, "What does Tobit say about giving to the poor?"),
];

function row(id: number, question: string): HistoryRow {
  return {
    id,
    asked_at: "2026-08-27T12:00:00Z",
    question,
    canon_mode: "66",
    model_id: "Qwen3-8B-Q4_K_M",
    index_version: "0.2.0",
    crisis_flag: false,
    verdict: "ok",
    fallback_used: false,
    preview: question,
    cited_count: 3,
  };
}

const settings: AppSettings = {
  canon: "66",
  model: "standard",
  compute: "auto",
  group_by: "book",
};

const info = {
  offline_statement: "It all happens on this computer.",
  crisis_note: "",
  paths: { user_db: "user.db" },
} as unknown as AppInfo;

function draw() {
  return render(
    <Main
      info={info}
      settings={settings}
      onSettingsChange={() => {}}
      onOpenSettings={() => {}}
      onOpenAbout={() => {}}
    />,
  );
}

/// Every delete control on screen, found the way a reader finds it: by the name
/// it announces, not by a class name only this test knows.
function deleteControls() {
  return screen.queryAllByRole("button", { name: "Delete this question" });
}

describe("the history sidebar", () => {
  let live: HistoryRow[];

  beforeEach(() => {
    live = [...rows];
    vi.spyOn(api, "onStage").mockResolvedValue(() => {});
    vi.spyOn(api, "historyList").mockImplementation(async () => live);
    vi.spyOn(api, "historySearch").mockImplementation(async () => live);
    vi.spyOn(api, "historyGet").mockResolvedValue(null);
    vi.spyOn(api, "historyDelete").mockImplementation(async (id: number) => {
      const before = live.length;
      live = live.filter((h) => h.id !== id);
      return live.length < before;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("gives every entry a delete control the reader can find and read", async () => {
    draw();
    await screen.findByText(rows[0].question);

    expect(deleteControls()).toHaveLength(3);

    // Named, so a screen reader announces it, and titled, so a mouse hovering
    // over it is told what it does. This is the pair Jared went looking for.
    for (const button of deleteControls()) {
      expect(button).toHaveProperty("title", "Delete this question");
      expect(button.getAttribute("aria-label")).toBe("Delete this question");
      // Not hidden behind a state that never arrives, and not disabled.
      expect(button.hasAttribute("hidden")).toBe(false);
      expect((button as HTMLButtonElement).disabled).toBe(false);
      // Something is actually drawn inside it.
      expect(button.querySelector("svg")).not.toBeNull();
    }
  });

  it("asks before it deletes, and asks on that entry only", async () => {
    draw();
    await screen.findByText(rows[1].question);

    fireEvent.click(deleteControls()[1]);

    // One entry is now asking; the other two still offer their delete.
    expect(screen.getByRole("button", { name: "Delete" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();
    expect(deleteControls()).toHaveLength(2);
    expect(api.historyDelete).not.toHaveBeenCalled();

    // Cancel puts it back and deletes nothing.
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(deleteControls()).toHaveLength(3);
    expect(api.historyDelete).not.toHaveBeenCalled();
  });

  it("deletes exactly that entry and leaves the others", async () => {
    draw();
    await screen.findByText(rows[1].question);

    fireEvent.click(deleteControls()[1]);
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => expect(deleteControls()).toHaveLength(2));

    expect(api.historyDelete).toHaveBeenCalledTimes(1);
    expect(api.historyDelete).toHaveBeenCalledWith(2);

    expect(screen.queryByText(rows[1].question)).toBeNull();
    expect(screen.getByText(rows[0].question)).toBeTruthy();
    expect(screen.getByText(rows[2].question)).toBeTruthy();
  });

  it("never opens the answer it is deleting", async () => {
    draw();
    await screen.findByText(rows[0].question);

    fireEvent.click(deleteControls()[0]);
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(api.historyDelete).toHaveBeenCalled());

    // Opening an entry is what `historyGet` is for, and deleting one is not
    // asking to read it. The delete sits beside the entry rather than inside
    // it precisely so that this click cannot do both.
    expect(api.historyGet).not.toHaveBeenCalled();
  });

  it("has no clear-all control: deleting everything lives in Settings", async () => {
    draw();
    await screen.findByText(rows[0].question);

    expect(screen.queryByRole("button", { name: /clear history/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /delete all/i })).toBeNull();
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });

  it("keeps the entry itself openable", async () => {
    draw();
    const first = await screen.findByText(rows[0].question);

    // The question text is inside the button that opens it, not inside the row.
    const opener = first.closest("button");
    expect(opener).not.toBeNull();
    expect(within(opener as HTMLElement).queryByRole("button")).toBeNull();

    fireEvent.click(opener as HTMLElement);
    await waitFor(() => expect(api.historyGet).toHaveBeenCalledWith(1));
  });
});
