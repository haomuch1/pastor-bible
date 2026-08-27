// The chapter a passage came from.
//
// A passage is a run of verses, and a run of verses is not always enough to
// judge what it says: the verse before it may be the condition, the verse after
// it the qualification. So every passage on the screen can be opened in its
// chapter, with the verses the answer rested on marked and scrolled to, and the
// chapters either side one press away.
//
// The verse text is read from index.db by a command, exactly as the answer's
// is. Nothing a model wrote reaches this screen. Closing it puts the reader back
// where they were: the answer underneath is never unmounted, so its scroll
// position, its expanded groups and its highlighted passage are all still there.

import { useEffect, useRef, useState } from "react";

import * as api from "../api";
import type { ChapterOut } from "../types";

interface Props {
  bookId: number;
  chapter: number;
  /// The verses to mark, which are the ones the answer rested on. The first of
  /// them is scrolled to.
  highlight: number[];
  onClose: () => void;
}

export function Reader({ bookId, chapter, highlight, onClose }: Props) {
  const [at, setAt] = useState({ bookId, chapter });
  const [text, setText] = useState<ChapterOut | null>(null);
  const [error, setError] = useState<string | null>(null);
  const body = useRef<HTMLDivElement | null>(null);

  // A new chapter opened from Previous or Next has no cited verses of its own;
  // only the chapter the reader came from is marked.
  const marked = new Set(
    at.bookId === bookId && at.chapter === chapter ? highlight : [],
  );

  useEffect(() => {
    let live = true;
    setText(null);
    setError(null);
    api
      .chapter(at.bookId, at.chapter)
      .then((c) => {
        if (!live) return;
        if (c) setText(c);
        else setError("That chapter is not in the Bible index.");
      })
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [at]);

  // Scroll to the first cited verse once the chapter is on screen, or to the
  // top of a chapter that has none.
  useEffect(() => {
    if (!text) return;
    const first = text.verses.find((v) => marked.has(v.verse_id));
    if (first) {
      document
        .getElementById(`read-${first.verse_id}`)
        ?.scrollIntoView({ block: "center" });
    } else {
      body.current?.scrollTo({ top: 0 });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text]);

  // Escape closes, and the arrow keys turn the page.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowLeft" && text?.previous) {
        setAt({ bookId: text.previous.book_id, chapter: text.previous.chapter });
      }
      if (e.key === "ArrowRight" && text?.next) {
        setAt({ bookId: text.next.book_id, chapter: text.next.chapter });
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [text, onClose]);

  return (
    <div className="overlay" onClick={onClose}>
      <div className="reader" onClick={(e) => e.stopPropagation()}>
        <header className="row between">
          <div className="row">
            <h2 style={{ margin: 0 }}>{text ? text.reference : "…"}</h2>
            {text?.canon === "deutero" && <span className="tag deutero">Deuterocanon</span>}
          </div>
          <button className="quiet" onClick={onClose}>
            Close
          </button>
        </header>

        <div className="reader-body" ref={body}>
          {error && <div className="err">{error}</div>}
          {!text && !error && <p className="faint">Reading…</p>}
          {text?.verses.map((v) => (
            <p
              key={v.verse_id}
              id={`read-${v.verse_id}`}
              className={marked.has(v.verse_id) ? "read-v is-cited" : "read-v"}
            >
              <span className="n">{v.reference.split(":").pop()}</span>
              {v.text}
            </p>
          ))}
        </div>

        <footer className="row between">
          <button
            className="quiet"
            disabled={!text?.previous}
            onClick={() =>
              text?.previous && setAt({ bookId: text.previous.book_id, chapter: text.previous.chapter })
            }
          >
            {text?.previous ? `← ${text.previous.reference}` : "←"}
          </button>
          <span className="faint">World English Bible</span>
          <button
            className="quiet"
            disabled={!text?.next}
            onClick={() =>
              text?.next && setAt({ bookId: text.next.book_id, chapter: text.next.chapter })
            }
          >
            {text?.next ? `${text.next.reference} →` : "→"}
          </button>
        </footer>
      </div>
    </div>
  );
}
