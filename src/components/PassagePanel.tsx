// The passages, which are the point.
//
// PLAN 7.2 as amended: the whole retrieved set is here, not just what the
// synopsis cited. Cited passages are marked and come first inside their group;
// the rest are behind a count, present rather than discarded. Verse text comes
// from index.db by way of the answer structure and never from the model.
//
// Grouping is by book, in canonical order, which is the order a reader already
// has in their head. Passages within a book run in chapter and verse order, and
// a cited passage is marked where it falls rather than lifted to the top: the
// point of Bible order is that it is Bible order.
//
// There is no other grouping. P4 found Nave's subtopics unusable as labels,
// some being whole paragraphs; P5 grouped by the root topic instead, which was
// a real improvement; P5.2 made book the default because the roots are not a
// category system either — an answer about giving to the poor was grouped under
// "HAMATH" and "TOB-ADONIJAH", roots that merely happen to contain a matching
// verse. P6 removes the switch: a heading that is not about what the passages
// have in common is worse than no heading, and keeping it as a second-best
// option only asks every reader to discover that for themselves.
//
// The matched topics are still in the answer (`Answer.topics` and
// `Answer.topic_groups`) and are still tested. Nothing about the data changed;
// this screen stopped offering it.

import { useMemo, useState } from "react";

import type { PassageOut } from "../types";

interface Props {
  passages: PassageOut[];
  highlight?: string | null;
  /// Open this passage's chapter in the reading view. A passage is a run of
  /// verses and a run of verses is not always enough to judge what it says.
  onRead?: (p: PassageOut) => void;
}

const BOOK_OF = (p: PassageOut) => p.reference.replace(/\s+\d+:.*$/, "");

export function PassagePanel({ passages, highlight, onRead }: Props) {
  const [openAll, setOpenAll] = useState(false);
  const [toggled, setToggled] = useState<Record<string, boolean>>({});

  const shown = useMemo(() => {
    const order: string[] = [];
    const map = new Map<string, PassageOut[]>();
    // Canonical order, which is the order the answer already carries: verse
    // ids ascend with the canon.
    const sorted = [...passages].sort((a, b) => (a.verse_ids[0] ?? 0) - (b.verse_ids[0] ?? 0));
    for (const p of sorted) {
      const book = BOOK_OF(p);
      if (!map.has(book)) {
        map.set(book, []);
        order.push(book);
      }
      map.get(book)!.push(p);
    }
    return order.map((book) => ({
      key: `book:${book}`,
      heading: book,
      items: map.get(book)!,
    }));
  }, [passages]);

  const citedCount = passages.filter((p) => p.cited).length;

  return (
    <div>
      <div className="row between" style={{ marginBottom: 8 }}>
        <h2 style={{ margin: 0 }}>
          Passages found
          <span className="faint" style={{ fontWeight: 400, marginLeft: 10 }}>
            {passages.length} in all, {citedCount} used in the answer
          </span>
        </h2>
        <div className="row">
          <button className="quiet" onClick={() => { setOpenAll(!openAll); setToggled({}); }}>
            {openAll ? "Collapse all" : "Expand all"}
          </button>
        </div>
      </div>

      {shown.map((g) => {
        const citedHere = g.items.filter((p) => p.cited);
        const hidden = g.items.length - citedHere.length;

        // A book the answer drew on opens; a book it did not stays shut. The
        // reader came for the passages behind the answer, and those are worth
        // scrolling past nothing to reach; the rest of the retrieved set is
        // there to be opened, not to be waded through.
        const openByDefault = citedHere.length > 0;
        const isOpen = openAll || (toggled[g.key] ?? openByDefault);

        // In Bible order from end to end, with the cited passages marked where
        // they fall rather than lifted to the top: the whole point of canonical
        // order is that it is canonical order.
        const shownItems = isOpen ? g.items : citedHere;

        return (
          <div className="group" key={g.key}>
            <h3>
              {g.heading}{" "}
              <span className="faint" style={{ fontWeight: 400 }}>
                {g.items.length}
              </span>
            </h3>
            {shownItems.map((p) => (
              <Passage
                key={p.reference}
                p={p}
                highlight={highlight === p.reference}
                onRead={onRead}
              />
            ))}
            {hidden > 0 && !isOpen && (
              <button className="quiet" onClick={() => setToggled({ ...toggled, [g.key]: true })}>
                Show {hidden} more passage{hidden === 1 ? "" : "s"}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

function Passage({
  p,
  highlight,
  onRead,
}: {
  p: PassageOut;
  highlight: boolean;
  onRead?: (p: PassageOut) => void;
}) {
  return (
    <div
      className={p.cited ? "passage is-cited" : "passage"}
      id={`passage-${p.reference.replace(/[^A-Za-z0-9]/g, "-")}`}
      style={highlight ? { outline: "2px solid var(--accent)" } : undefined}
    >
      <div className="row between">
        <span className="ref">{p.reference}</span>
        <span className="row">
          {p.cited && <span className="tag cited">In the answer</span>}
          {p.canon === "deutero" && <span className="tag deutero">Deuterocanon</span>}
          {onRead && (
            <button className="quiet" onClick={() => onRead(p)}>
              Read chapter
            </button>
          )}
        </span>
      </div>
      <div className="verses">
        {p.verses.map((v) => (
          <div className="v" key={v.verse_id}>
            <span className="n">{v.reference.split(":").pop()}</span>
            {v.text}
          </div>
        ))}
      </div>
      {p.origins.length > 0 && (
        <div className="origins">
          {p.origins.map((o) => (
            <span className="tag" key={o}>
              {ORIGIN_LABEL[o] ?? o}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

// The origin tags say how a passage was found. A reader who wants to know why
// something is on the list should be able to see it without a manual.
const ORIGIN_LABEL: Record<string, string> = {
  "vector-verse": "meaning, verse",
  "vector-pericope": "meaning, paragraph",
  fts: "wording",
  topic: "Nave's topic",
  tsk: "cross-reference",
};
