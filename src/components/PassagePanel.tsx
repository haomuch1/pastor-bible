// The passages, which are the point.
//
// PLAN 7.2 as amended: the whole retrieved set is here, not just what the
// synopsis cited. Cited passages are marked and come first inside their group;
// the rest are behind a count, present rather than discarded. Verse text comes
// from index.db by way of the answer structure and never from the model.
//
// Grouping is by the root Nave's topic by default, with the matched subtopic as
// a second line, and by book on the switch. P4 flagged that Nave's subtopic
// headings are unusable as labels because some are whole paragraphs, so the
// root heading is what a group is called.

import { useMemo, useState } from "react";

import type { PassageOut, TopicGroup } from "../types";

interface Props {
  passages: PassageOut[];
  groups: TopicGroup[];
  groupBy: "topic" | "book";
  onGroupByChange: (v: "topic" | "book") => void;
  highlight?: string | null;
}

const BOOK_OF = (p: PassageOut) => p.reference.replace(/\s+\d+:.*$/, "");

export function PassagePanel({ passages, groups, groupBy, onGroupByChange, highlight }: Props) {
  const [openAll, setOpenAll] = useState(false);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const byRef = useMemo(() => {
    const m = new Map<string, PassageOut>();
    for (const p of passages) m.set(p.reference, p);
    return m;
  }, [passages]);

  const shown = useMemo(() => {
    if (groupBy === "book") {
      const order: string[] = [];
      const map = new Map<string, PassageOut[]>();
      // Canonical order, which is the order the answer already carries: verse
      // ids ascend with the canon.
      const sorted = [...passages].sort(
        (a, b) => (a.verse_ids[0] ?? 0) - (b.verse_ids[0] ?? 0),
      );
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
        sub: null as string | null,
        items: map.get(book)!,
      }));
    }
    return groups.map((g) => ({
      key: `topic:${g.topic_id ?? "other"}`,
      heading: g.heading_display || "Other passages",
      // Nave's topic that matched, beneath the root the group is named for.
      sub: g.heading ? `matched under ${g.heading}` : null,
      items: g.passage_refs.map((r) => byRef.get(r)).filter(Boolean) as PassageOut[],
    }));
  }, [groupBy, groups, passages, byRef]);

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
          <span className="faint">Group by</span>
          <button
            className={groupBy === "topic" ? "choice on" : "choice"}
            onClick={() => onGroupByChange("topic")}
          >
            Topic
          </button>
          <button
            className={groupBy === "book" ? "choice on" : "choice"}
            onClick={() => onGroupByChange("book")}
          >
            Book
          </button>
          <button className="quiet" onClick={() => { setOpenAll(!openAll); setExpanded({}); }}>
            {openAll ? "Collapse all" : "Expand all"}
          </button>
        </div>
      </div>

      {shown.map((g) => {
        const cited = g.items.filter((p) => p.cited);
        const rest = g.items.filter((p) => !p.cited);
        const isOpen = openAll || expanded[g.key];
        return (
          <div className="group" key={g.key}>
            <h3>
              {g.heading}{" "}
              <span className="faint" style={{ fontWeight: 400 }}>
                {g.items.length}
              </span>
            </h3>
            {g.sub && <div className="sub">{g.sub}</div>}
            {cited.map((p) => (
              <Passage key={p.reference} p={p} highlight={highlight === p.reference} />
            ))}
            {rest.length > 0 && !isOpen && (
              <button
                className="quiet"
                onClick={() => setExpanded({ ...expanded, [g.key]: true })}
              >
                Show {rest.length} more passage{rest.length === 1 ? "" : "s"}
              </button>
            )}
            {isOpen &&
              rest.map((p) => (
                <Passage key={p.reference} p={p} highlight={highlight === p.reference} />
              ))}
          </div>
        );
      })}
    </div>
  );
}

function Passage({ p, highlight }: { p: PassageOut; highlight: boolean }) {
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
