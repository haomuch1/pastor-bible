// The verified synopsis, rendered.
//
// This is deliberately not a markdown library. The synopsis has exactly two
// shapes in it, "## heading" lines and paragraphs, and the only other thing it
// contains is [P#] tokens. Rendering it by hand means no model output is ever
// interpreted as markup: there is no path by which a link, an image or a script
// could come out of an answer, because nothing but headings, text and tokens is
// ever produced.
//
// The [P#] tokens become chips showing the passage's reference, and a
// deuterocanonical passage is marked here from `canon` rather than from
// anything the model wrote. P4 measured the model dropping the marker it was
// told to keep; docs/API.md records why this is the caller's job.

import type { PassageOut } from "../types";

interface Props {
  markdown: string;
  passages: PassageOut[];
  onSelect?: (reference: string) => void;
}

const TOKEN = /(\[P\d+\])/g;

export function Synopsis({ markdown, passages, onSelect }: Props) {
  const byToken = new Map<string, PassageOut>();
  for (const p of passages) if (p.token) byToken.set(p.token, p);

  const blocks: { heading: string | null; lines: string[] }[] = [];
  for (const raw of markdown.split("\n")) {
    const line = raw.trimEnd();
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    if (heading) {
      blocks.push({ heading: heading[1].trim(), lines: [] });
    } else if (line.trim() === "") {
      if (blocks.length) blocks[blocks.length - 1].lines.push("");
    } else {
      if (!blocks.length) blocks.push({ heading: null, lines: [] });
      blocks[blocks.length - 1].lines.push(line);
    }
  }

  return (
    <div className="synopsis">
      {blocks.map((b, i) => (
        <section key={i}>
          {b.heading && <h2>{b.heading}</h2>}
          {paragraphs(b.lines).map((para, j) => (
            <p key={j}>{inline(para, byToken, onSelect)}</p>
          ))}
        </section>
      ))}
    </div>
  );
}

function paragraphs(lines: string[]): string[] {
  const out: string[] = [];
  let current: string[] = [];
  for (const line of lines) {
    if (line.trim() === "") {
      if (current.length) out.push(current.join(" "));
      current = [];
    } else {
      current.push(line.trim());
    }
  }
  if (current.length) out.push(current.join(" "));
  return out;
}

function inline(
  text: string,
  byToken: Map<string, PassageOut>,
  onSelect?: (reference: string) => void,
) {
  return text.split(TOKEN).map((part, i) => {
    if (!TOKEN.test(part)) {
      TOKEN.lastIndex = 0;
      return <span key={i}>{part}</span>;
    }
    TOKEN.lastIndex = 0;
    const p = byToken.get(part);
    if (!p) {
      // Cannot happen: the verifier rejects any token that was not sent, and
      // an answer that reaches here has passed it. Shown plainly if it ever
      // does, rather than silently dropped.
      return (
        <span key={i} className="faint">
          {part}
        </span>
      );
    }
    const deutero = p.canon === "deutero";
    return (
      <button
        key={i}
        type="button"
        className={deutero ? "chip deutero" : "chip"}
        title={deutero ? `${p.reference} — Deuterocanon` : p.reference}
        onClick={() => onSelect?.(p.reference)}
      >
        {p.reference}
        {deutero ? " · Deuterocanon" : ""}
      </button>
    );
  });
}
