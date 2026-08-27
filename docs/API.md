# API

The structure the backend returns for one question. Defined as
`pastor_bible_core::api::Answer` in src-tauri/core/src/api.rs; this document is
what it means and why it is shaped this way. P5's frontend consumes it and
`user.db` stores parts of it as history.

Two rules shape the type, and both exist so that a caller which forgets to
check a field still cannot break the promise:

- **Verse text always comes from index.db.** `passages[].verses[].text` is read
  from the database at assembly time. Nothing a model wrote reaches it, at any
  stage, the fallback included.
- **`synopsis_markdown` is populated only when the verifier passed.** When it
  did not, that field is `null` and `fallback_markdown` carries the grouped
  passages instead. There is no field that holds an unverified synopsis.

## Fields

    question              string   the question as the reader typed it
    canon_mode            "66" | "both"
    crisis                bool     PLAN 5.8 matched the question
    crisis_note           string?  PLAN 9.3 verbatim, present only when crisis
                                   is true. Shown ABOVE the answer, never
                                   instead of it, and the answer still runs.

    synopsis_markdown     string?  the verified themed synopsis
    fallback_markdown     string?  PLAN 5.6's fallback: passages grouped by
                                   book with the one-line note. Present only
                                   when synopsis_markdown is null.
    verdict               "ok" | "violation" | "fallback"
    fallback_used         bool
    attempts              array    one entry per generation, in order:
                                     verdict, seconds, prompt_tokens,
                                     completion_tokens, violations[]
                                   violations carry kind ("token" |
                                   "reference"), text, reason, and span as
                                   character offsets into that attempt's text.

    cited_tokens          [string] the [P#] tokens the synopsis cites, sorted
    cited_passage_ids     [int]    the verse ids behind those tokens
    deuterocanon_cited    bool
    deuterocanon_footer   string?  PLAN 5.7's one-line footer, when it applies

    passages              array    EVERY passage retrieved, in rank order:
                                     token          the [P#] it was sent as,
                                                    null if it was not sent
                                     reference      "Mat 6:24-34"
                                     verse_ids      [int]
                                     verses         [{verse_id, reference, text}]
                                     score          fused retrieval score
                                     origins        ["fts","topic","tsk",
                                                     "vector-pericope",
                                                     "vector-verse"]
                                     canon          "protestant" | "deutero"
                                     cited          the synopsis cites it
                                     sent           it was sent to the model
    sent_count            int      how many were sent

    topics                array    matched Nave's topics, in match order:
                                     topic_id, heading, heading_display,
                                     verses (how many the topic holds in
                                     Nave's, not how many were retrieved),
                                     score, passage_refs
    topic_groups          array    PLAN 5.6 as amended 2026-08-26: the full
                                   set grouped under the matched topic
                                   headings, topics in match order, passages
                                   within a topic in canonical order, and
                                   everything else under "Other passages".
                                   The groups partition the set: a passage in
                                   two topics is listed under the stronger
                                   one, so the reader is not shown it twice.
                                   No model call is involved.

    timings               object   seconds per stage: index_load,
                                   embed_server, embed, chat_server, retrieve,
                                   generate, retry, verify, total
    model_id              string   the chat model's file stem
    embedding_model_id    string
    index_version         string   from index.db's meta
    prompt_versions       [[name, version]]
    sidecar_path          "sequential" | "concurrent"
    peak_ram_mb           number?  peak resident memory of the sidecar
    query_mode            "raw" | "rewrite" | "fused"

## Notes on two fields that are easy to misread

`topics[].verses` is the size of the topic in Nave's, which is usually far
larger than `topics[].passage_refs`. A topic of 492 verses that contributed 18
passages is normal; showing the first number as though it were the second would
tell the reader the app found 492 passages.

`heading_display` exists because Nave's writes some subtopics as full sentences
and a few as whole paragraphs, the longest over a thousand tokens. P2 recorded
this and decided not to re-derive them. `heading` is the source text;
`heading_display` is a label that fits above a list. P5 should use the second
and can offer the first.

## Stability

This is not a public API. It changes when the product changes, and P5 is free
to ask for more. The two rules at the top are not negotiable, because they are
how PLAN 5.6 is kept.
