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
- **Every reference a reader sees is spelled the way a reader writes it.**
  `passages[].reference`, `verses[].reference`, `topics[].passage_refs` and
  `topic_groups[].passage_refs` are all "1 Kings 3:9", never "1Ki 3:9". The
  compact form exists in one place only, the prompt sent to the model, and it
  never leaves `Engine::pack`. See "Two spellings of a book" below.
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
                                     reference      "Matthew 6:24-34"
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

## Two spellings of a book

index.db's `books` table carries two, and neither is what belongs on a citation
chip. `abbrev` is "1Ki", which is the compact form the Treasury of Scripture
Knowledge speaks and which the committed retrieval fixtures hold. `name` is the
World English Bible's own running title, which for the same book is "The First
Book of Kings" and for one deuterocanonical book runs to sixteen words.

So there is a third, `index::DISPLAY_NAMES`, keyed on USFM code: "1 Kings". It
is the only book data in this program that is not read from the index, and
`src-tauri/core/tests/book_names.rs` asserts that every book index.db holds has
an entry in it and that no abbreviation reaches any screen or any exported file.

The compact form is deliberately still what the prompt sends, because that is
what P3 and P4 measured and what the fixtures pin. The same test asserts that
too, so the split cannot close in either direction by accident.

## The Deuterocanon tag is the caller's job

PLAN 5.7 requires a visible "Deuterocanon" tag on every deuterocanonical
passage in the synopsis as well as in the panel. The synopsis prompt asks the
model to keep the marker it was given, and on 2026-08-26 the 8B cited Tob 4:7
in a both-canon answer and dropped it. So the tag is rendered by the caller
from `passages[].canon` when it draws the cited tokens, in exactly the way
verse text is read from the index rather than from the answer. The prompt still
asks; nothing rests on the asking.

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

## The commands the window may call

Every one is a Tauri command; the frontend has no other way to reach anything,
and it keeps nothing of its own. There is no browser storage in this app: every
setting and every answer is in user.db.

    app_info()                     version, index version, the disclaimer and
                                   crisis note read from their source files,
                                   credits, licences, paths
    hardware_check()               this machine beside the reference machine
    startup_state()                first run or not, model files present,
                                   settings, the last self-test, history count,
                                   and `model_problem`: a plain sentence naming
                                   a model file that is missing or is not the
                                   file we pinned, or null. `ask` returns the
                                   same sentence rather than running.
    get_settings() / set_setting(key, value)
    download_model(id)             emits "download-progress"
    cancel_download()
    ask(question)                  emits "ask-stage"; returns the Answer above
                                   and saves it to history
    retrieved_passages()           the passages for the question being
                                   answered, collected once, as soon as the
                                   "retrieved" stage arrives
    cancel_ask()
    run_self_test()                three canned questions end to end
    finish_first_run()
    history_list / history_search / history_get / history_delete /
    history_clear
    history_export(path, format)  format is "txt" for the plain-text copy or
                                  "xlsx" for the workbook: one sheet listing
                                  every question, then one per question with
                                  its answer and every passage it rested on,
                                  verse text included. Both are written from
                                  the same reader in user.db and both read
                                  their verse text from index.db. A question
                                  whose entry was written against a different
                                  index version says so and lists references
                                  without text.
    chapter(book_id, chapter)      the whole chapter a passage came from,
                                   for the reading view: the verses from
                                   index.db, the book named as a reader writes
                                   it, the canon tag, and the chapters either
                                   side. Previous and next follow the reader's
                                   canon setting; the chapter asked for always
                                   opens, whatever the setting, because a
                                   citation being followed must resolve.
    crisis_note()
    shutdown_models()

### Two events

`ask-stage` carries what the reader is waiting for: `loading_model`,
`retrieving`, `retrieved`, `generating` with a running token count,
`checking_references`, `retrying`, `done`, `cancelled`, `failed`. It never
carries the text being generated. PLAN 5.6 forbids showing an unverified
reference, and a token stream is exactly that.

`download-progress` carries `checking`, `downloading` with bytes, rate and an
estimate, `verifying`, `done`, `failed`.

### Why the passages are fetched and not pushed

The `retrieved` stage says only how many passages there are. The passages
themselves are about a quarter of a megabyte and are collected with
`retrieved_passages()`. A payload that size does not survive the event channel:
measured on 2026-08-26, the counts arrived and the list did not, silently, while
the same data returns without trouble from a command. So the event is the
signal and the command is the delivery.

`retrieved_passages()` deliberately does not touch the session lock, because
`ask` holds that for the whole two and a half minutes of an answer and this has
to be answered during one.

## Stability

This is not a public API. It changes when the product changes, and P5 is free
to ask for more. The two rules at the top are not negotiable, because they are
how PLAN 5.6 is kept.
