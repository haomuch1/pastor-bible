//! Retrieval, PLAN 5.1 to 5.4, configuration F.
//!
//! This is a port of pipeline/retrieve.py, and it is a port in the strict
//! sense: the fixtures in core/tests/fixtures hold the Python harness's output
//! for the graded questions, and the parity test requires this code to
//! reproduce them exactly. Where an implementation choice was free, the choice
//! that reproduces Python was taken, and the reason is written down at the
//! place it matters. Insertion order is one of those places: Python's dicts
//! preserve it, and the TSK anchor list and the TSK keep-list both depend on
//! it, so the score map here is insertion-ordered too.

use std::collections::{HashMap, HashSet};

use crate::index::{verse_book, verse_chapter, verse_num, Index};

pub const RRF_K: f64 = 60.0;
pub const TOPIC_CAP: i64 = 60;
pub const TSK_CAP: usize = 200;
pub const TOPIC_HITS: usize = 5;
pub const TSK_ANCHORS: usize = 25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CanonMode {
    Protestant66,
    DeuteroOnly,
    Both,
}

impl CanonMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "66" => Ok(CanonMode::Protestant66),
            "deutero-only" => Ok(CanonMode::DeuteroOnly),
            "both" => Ok(CanonMode::Both),
            _ => Err(format!("unknown canon mode {:?}", s)),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            CanonMode::Protestant66 => "66",
            CanonMode::DeuteroOnly => "deutero-only",
            CanonMode::Both => "both",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub verse_id: i64,
    pub score: f64,
    pub origins: Vec<String>,
    pub canon: String,
}

#[derive(Clone, Debug)]
pub struct Passage {
    /// The compact form, "1Ki 3:9". This is what the retrieval parity fixtures
    /// hold and what the prompt sends to the model, and it is never shown to a
    /// reader; `display_reference` is.
    pub reference: String,
    /// The same passage as a reader writes it: "1 Kings 3:9". Everything the
    /// window draws and everything an exported file contains uses this.
    pub display_reference: String,
    pub verse_ids: Vec<i64>,
    pub score: f64,
    pub origins: Vec<String>,
    pub canon: String,
}

#[derive(Clone, Debug)]
pub struct MatchedTopic {
    pub topic_id: i64,
    pub heading: String,
    pub verses: i64,
    pub score: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub vector_verses: bool,
    pub vector_pericopes: bool,
    pub fts: bool,
    pub topics: bool,
    pub tsk: bool,
}

impl Config {
    /// Configuration F: the shipping configuration, chosen in P2.
    pub fn f() -> Self {
        Config { vector_verses: true, vector_pericopes: true, fts: true, topics: true, tsk: true }
    }
}

/// An insertion-ordered f64 accumulator keyed by verse id.
///
/// Python dicts preserve insertion order and two stages depend on it: the TSK
/// anchor list is a stable sort over this map, and so is the TSK keep-list. A
/// HashMap would reorder both and the parity fixtures would not reproduce.
struct Scores {
    idx: HashMap<i64, usize>,
    keys: Vec<i64>,
    vals: Vec<f64>,
}

impl Scores {
    fn new() -> Self {
        Scores { idx: HashMap::new(), keys: Vec::new(), vals: Vec::new() }
    }
    fn add(&mut self, key: i64, delta: f64) {
        match self.idx.get(&key) {
            Some(&i) => self.vals[i] += delta,
            None => {
                self.idx.insert(key, self.keys.len());
                self.keys.push(key);
                self.vals.push(delta);
            }
        }
    }
    fn iter(&self) -> impl Iterator<Item = (i64, f64)> + '_ {
        self.keys.iter().copied().zip(self.vals.iter().copied())
    }
    fn len(&self) -> usize {
        self.keys.len()
    }
}

/// The same shape as Scores, for integer counts.
struct Counts {
    idx: HashMap<i64, usize>,
    keys: Vec<i64>,
    vals: Vec<i64>,
}

impl Counts {
    fn new() -> Self {
        Counts { idx: HashMap::new(), keys: Vec::new(), vals: Vec::new() }
    }
    fn add(&mut self, key: i64, delta: i64) {
        match self.idx.get(&key) {
            Some(&i) => self.vals[i] += delta,
            None => {
                self.idx.insert(key, self.keys.len());
                self.keys.push(key);
                self.vals.push(delta);
            }
        }
    }
    fn items(&self) -> Vec<(i64, i64)> {
        self.keys.iter().copied().zip(self.vals.iter().copied()).collect()
    }
}

pub struct Retriever {
    pub index: Index,
    pub model_id: String,
    pub dim: usize,
    pub query_prefix: String,
    pub doc_prefix: String,
    pub n_ctx: i64,
    pub gguf_file: String,
    verse_keys: Vec<i64>,
    verse_mat: Vec<f32>,
    peri_meta: Vec<(i64, i64, i64, i64)>,
    peri_mat: Vec<f32>,
    topic_keys: Vec<i64>,
    topic_mat: Vec<f32>,
}

fn blob_to_f32(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

impl Retriever {
    pub fn open(db_path: &str, model_id: &str) -> Result<Self, String> {
        let index = Index::open(db_path)?;
        let (dim, doc_prefix, query_prefix, n_ctx, gguf_file): (i64, String, String, i64, String) =
            index
                .con
                .query_row(
                    "SELECT dim, doc_prefix, query_prefix, n_ctx, gguf_file FROM embedding_models \
                     WHERE model_id = ?",
                    [model_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .map_err(|_| format!("no embeddings for model {:?}", model_id))?;
        let dim = dim as usize;

        let mut verse_keys = Vec::new();
        let mut verse_mat: Vec<f32> = Vec::new();
        {
            let mut st = index
                .con
                .prepare(
                    "SELECT verse_id, vec FROM verse_embeddings WHERE model_id = ? \
                     ORDER BY verse_id",
                )
                .map_err(|e| e.to_string())?;
            let mut rows = st.query([model_id]).map_err(|e| e.to_string())?;
            while let Some(r) = rows.next().map_err(|e| e.to_string())? {
                verse_keys.push(r.get::<_, i64>(0).map_err(|e| e.to_string())?);
                let blob: Vec<u8> = r.get(1).map_err(|e| e.to_string())?;
                verse_mat.extend_from_slice(&blob_to_f32(&blob));
            }
        }

        let mut peri_meta = Vec::new();
        let mut peri_mat: Vec<f32> = Vec::new();
        {
            let mut st = index
                .con
                .prepare(
                    "SELECT pericope_id, part, start_verse_id, end_verse_id, vec \
                     FROM pericope_embeddings WHERE model_id = ? ORDER BY pericope_id, part",
                )
                .map_err(|e| e.to_string())?;
            let mut rows = st.query([model_id]).map_err(|e| e.to_string())?;
            while let Some(r) = rows.next().map_err(|e| e.to_string())? {
                peri_meta.push((
                    r.get::<_, i64>(0).map_err(|e| e.to_string())?,
                    r.get::<_, i64>(1).map_err(|e| e.to_string())?,
                    r.get::<_, i64>(2).map_err(|e| e.to_string())?,
                    r.get::<_, i64>(3).map_err(|e| e.to_string())?,
                ));
                let blob: Vec<u8> = r.get(4).map_err(|e| e.to_string())?;
                peri_mat.extend_from_slice(&blob_to_f32(&blob));
            }
        }

        let mut topic_keys = Vec::new();
        let mut topic_mat: Vec<f32> = Vec::new();
        {
            let mut st = index
                .con
                .prepare(
                    "SELECT topic_id, vec FROM topic_embeddings WHERE model_id = ? \
                     ORDER BY topic_id",
                )
                .map_err(|e| e.to_string())?;
            let mut rows = st.query([model_id]).map_err(|e| e.to_string())?;
            while let Some(r) = rows.next().map_err(|e| e.to_string())? {
                topic_keys.push(r.get::<_, i64>(0).map_err(|e| e.to_string())?);
                let blob: Vec<u8> = r.get(1).map_err(|e| e.to_string())?;
                topic_mat.extend_from_slice(&blob_to_f32(&blob));
            }
        }

        Ok(Retriever {
            index,
            model_id: model_id.to_string(),
            dim,
            query_prefix,
            doc_prefix,
            n_ctx,
            gguf_file,
            verse_keys,
            verse_mat,
            peri_meta,
            peri_mat,
            topic_keys,
            topic_mat,
        })
    }

    fn canon_ok(&self, verse_id: i64, mode: CanonMode) -> bool {
        let canon = self.index.canon_of_verse(verse_id);
        match mode {
            CanonMode::Protestant66 => canon == "protestant",
            CanonMode::DeuteroOnly => canon == "deutero",
            CanonMode::Both => true,
        }
    }

    /// Cosine against a unit-normalised query, brute force. The vectors in the
    /// index are unit-normalised at build time, so the dot product is the
    /// cosine and no division is needed.
    ///
    /// The accumulator is f64 even though both operands are f32. The first
    /// version summed in f32 and put Pro 2:5 above Psa 19:9 for question g08,
    /// where the Python harness has them the other way round. Their true
    /// cosines differ by 1.8e-7, so the order is decided entirely by rounding:
    /// numpy's BLAS sums in blocks and lands within 1e-9 of the exact value,
    /// while a sequential f32 sum drifts 2.4e-7 and crosses the gap. Summing in
    /// f64 is both the more accurate answer and the one that reproduces the
    /// harness. It is not a guarantee for every possible query: two passages
    /// this close together are a coin toss that no implementation can settle,
    /// and the parity claim is over the fixtures, which is what was measured.
    fn scores_against(mat: &[f32], dim: usize, q: &[f32]) -> Vec<f64> {
        let n = mat.len() / dim;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let row = &mat[i * dim..(i + 1) * dim];
            let mut acc = 0f64;
            for k in 0..dim {
                acc += (row[k] as f64) * (q[k] as f64);
            }
            out.push(acc);
        }
        out
    }

    /// Indices of the highest `keep` scores, best first, ties by index.
    ///
    /// numpy breaks ties by whatever argpartition happened to produce. Ties in
    /// a cosine score are vanishingly rare, and breaking them by index is the
    /// one rule that is both deterministic and reproducible on the Python side.
    fn top(scores: &[f64], keep: usize) -> Vec<usize> {
        let keep = keep.min(scores.len());
        if keep == 0 {
            return Vec::new();
        }
        let cmp = |a: &usize, b: &usize| {
            scores[*b].partial_cmp(&scores[*a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b))
        };
        let mut idx: Vec<usize> = (0..scores.len()).collect();
        idx.select_nth_unstable_by(keep - 1, cmp);
        idx.truncate(keep);
        idx.sort_by(cmp);
        idx
    }

    pub fn vector_verses(&self, q: &[f32], mode: CanonMode, limit: usize) -> Vec<(f64, i64)> {
        let scores = Self::scores_against(&self.verse_mat, self.dim, q);
        let mut out = Vec::new();
        for i in Self::top(&scores, limit * 4) {
            let vid = self.verse_keys[i];
            if !self.canon_ok(vid, mode) {
                continue;
            }
            out.push((scores[i], vid));
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    pub fn vector_pericopes(
        &self,
        q: &[f32],
        mode: CanonMode,
        limit: usize,
    ) -> Vec<(f64, Vec<i64>)> {
        let scores = Self::scores_against(&self.peri_mat, self.dim, q);
        let mut st = self
            .index
            .con
            .prepare_cached(
                "SELECT verse_id FROM verses WHERE verse_id BETWEEN ? AND ? ORDER BY verse_id",
            )
            .expect("pericope verses prepare");
        let mut out = Vec::new();
        for i in Self::top(&scores, limit * 4) {
            let (_pid, _part, sv, ev) = self.peri_meta[i];
            if !self.canon_ok(sv, mode) {
                continue;
            }
            let vids: Vec<i64> = st
                .query_map([sv, ev], |r| r.get::<_, i64>(0))
                .expect("pericope verses query")
                .filter_map(|r| r.ok())
                .collect();
            out.push((scores[i], vids));
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    pub fn vector_topics(&self, q: &[f32], limit: usize) -> Vec<(f64, i64)> {
        let scores = Self::scores_against(&self.topic_mat, self.dim, q);
        Self::top(&scores, limit).into_iter().map(|i| (scores[i], self.topic_keys[i])).collect()
    }

    /// FTS5 keyword search, one ranked list per term.
    pub fn fts(&self, terms: &[String], mode: CanonMode, limit: usize) -> Vec<Vec<i64>> {
        let mut ranked = Vec::new();
        for term in terms {
            let t = if term.contains(' ') {
                format!("\"{}\"", term.replace('"', ""))
            } else {
                term.clone()
            };
            let mut sql = String::from(
                "SELECT f.rowid FROM verse_fts f JOIN verses v ON v.verse_id = f.rowid \
                 JOIN books b ON b.book_id = v.book_id WHERE verse_fts MATCH ?",
            );
            match mode {
                CanonMode::Protestant66 => sql.push_str(" AND b.canon = 'protestant'"),
                CanonMode::DeuteroOnly => sql.push_str(" AND b.canon = 'deutero'"),
                CanonMode::Both => {}
            }
            sql.push_str(" ORDER BY bm25(verse_fts) LIMIT ?");
            // A malformed MATCH term yields an empty list rather than an error,
            // exactly as the Python harness does.
            let ids = (|| -> rusqlite::Result<Vec<i64>> {
                let mut st = self.index.con.prepare_cached(&sql)?;
                let rows =
                    st.query_map(rusqlite::params![t, limit as i64], |r| r.get::<_, i64>(0))?;
                rows.collect()
            })()
            .unwrap_or_default();
            ranked.push(ids);
        }
        ranked
    }

    fn topic_expand(&self, topic_ids: &[i64], mode: CanonMode) -> Counts {
        let mut out = Counts::new();
        let mut st = self
            .index
            .con
            .prepare_cached("SELECT verse_id FROM nave_topic_verses WHERE topic_id = ? LIMIT ?")
            .expect("topic_expand prepare");
        for &tid in topic_ids {
            let vids: Vec<i64> = st
                .query_map(rusqlite::params![tid, TOPIC_CAP], |r| r.get::<_, i64>(0))
                .expect("topic_expand query")
                .filter_map(|r| r.ok())
                .collect();
            for v in vids {
                if self.canon_ok(v, mode) {
                    out.add(v, 1);
                }
            }
        }
        out
    }

    fn tsk_expand(&self, anchors: &[i64], mode: CanonMode) -> Vec<(i64, i64)> {
        if anchors.is_empty() {
            return Vec::new();
        }
        let marks = vec!["?"; anchors.len()].join(",");
        let sql =
            format!("SELECT to_verse_id FROM tsk_refs WHERE from_verse_id IN ({}) LIMIT ?", marks);
        let mut st = self.index.con.prepare(&sql).expect("tsk_expand prepare");
        let mut params: Vec<i64> = anchors.to_vec();
        params.push((TSK_CAP * 4) as i64);
        let mut out = Counts::new();
        let rows = st
            .query_map(rusqlite::params_from_iter(params.iter()), |r| r.get::<_, i64>(0))
            .expect("tsk_expand query");
        for tid in rows.filter_map(|r| r.ok()) {
            if self.canon_ok(tid, mode) {
                out.add(tid, 1);
            }
        }
        // Python: sorted(out.items(), key=lambda kv: -kv[1])[:TSK_CAP], a stable
        // sort over insertion order.
        let mut items = out.items();
        items.sort_by(|a, b| b.1.cmp(&a.1));
        items.truncate(TSK_CAP);
        items
    }

    /// PLAN 5.1 to 5.4. Returns (full set, top cut, matched topics).
    ///
    /// In both-canon mode retrieval is additive: the canon-66 search runs
    /// unchanged, a Deuterocanon-only search runs beside it, and its best
    /// `deutero_slice` are appended below the protestant tail. The 66 result is
    /// therefore always a prefix of the both result and the toggle can only
    /// add. P2 measured that simply unfiltering displaced up to 15 of 25
    /// protestant passages.
    pub fn search(
        &self,
        qvec: &[f32],
        keywords: &[String],
        mode: CanonMode,
        cfg: Config,
        top_n: usize,
        pool: usize,
        deutero_slice: usize,
    ) -> (Vec<Candidate>, Vec<Candidate>, Vec<MatchedTopic>) {
        if mode == CanonMode::Both {
            let (mut base_full, base_top, topics) =
                self.search_one(qvec, keywords, CanonMode::Protestant66, cfg, top_n, pool);
            let (deut_full, _, _) =
                self.search_one(qvec, keywords, CanonMode::DeuteroOnly, cfg, top_n, pool);
            let floor = base_full.last().map(|c| c.score).unwrap_or(0.0);
            let mut extra: Vec<Candidate> = Vec::new();
            for (i, c) in deut_full.iter().take(deutero_slice).enumerate() {
                let mut c = c.clone();
                c.score = round_to(floor - 1e-6 * ((i + 1) as f64), 9);
                extra.push(c);
            }
            base_full.extend(extra.iter().cloned());
            let mut top: Vec<Candidate> = base_top.into_iter().take(top_n).collect();
            top.extend(extra);
            return (base_full, top, topics);
        }
        self.search_one(qvec, keywords, mode, cfg, top_n, pool)
    }

    fn search_one(
        &self,
        qvec: &[f32],
        keywords: &[String],
        mode: CanonMode,
        cfg: Config,
        top_n: usize,
        pool: usize,
    ) -> (Vec<Candidate>, Vec<Candidate>, Vec<MatchedTopic>) {
        let mut lists: Vec<Vec<i64>> = Vec::new();
        let mut origins: HashMap<i64, HashSet<&'static str>> = HashMap::new();

        if cfg.vector_verses {
            let hits = self.vector_verses(qvec, mode, pool);
            let ids: Vec<i64> = hits.iter().map(|(_, v)| *v).collect();
            for &vid in &ids {
                origins.entry(vid).or_default().insert("vector-verse");
            }
            lists.push(ids);
        }

        if cfg.vector_pericopes {
            let mut flat: Vec<i64> = Vec::new();
            let mut seen: HashSet<i64> = HashSet::new();
            for (_, vids) in self.vector_pericopes(qvec, mode, pool) {
                for vid in vids {
                    if seen.insert(vid) {
                        flat.push(vid);
                    }
                    // Python tags every verse of every returned pericope, not
                    // only the ones that survive the cut below.
                    origins.entry(vid).or_default().insert("vector-pericope");
                }
            }
            flat.truncate(pool * 4);
            lists.push(flat);
        }

        if cfg.fts {
            for ids in self.fts(keywords, mode, pool) {
                for &vid in &ids {
                    origins.entry(vid).or_default().insert("fts");
                }
                lists.push(ids);
            }
        }

        // Reciprocal rank fusion, weight 1.0 on every list.
        let mut score = Scores::new();
        for lst in &lists {
            for (pos, &vid) in lst.iter().enumerate() {
                score.add(vid, 1.0 / (RRF_K + (pos as f64) + 1.0));
            }
        }

        let mut matched_topics = Vec::new();
        if cfg.topics {
            let top = self.vector_topics(qvec, TOPIC_HITS);
            let tids: Vec<i64> = top.iter().map(|(_, t)| *t).collect();
            for (s, tid) in &top {
                let row: rusqlite::Result<(String, i64)> = self.index.con.query_row(
                    "SELECT heading, (SELECT COUNT(*) FROM nave_topic_verses v \
                     WHERE v.topic_id = t.topic_id) FROM nave_topics t WHERE topic_id = ?",
                    [tid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                );
                if let Ok((heading, verses)) = row {
                    matched_topics.push(MatchedTopic {
                        topic_id: *tid,
                        heading,
                        verses,
                        score: round_to(*s, 4),
                    });
                }
            }
            for (vid, n) in self.topic_expand(&tids, mode).items() {
                origins.entry(vid).or_default().insert("topic");
                score.add(vid, 0.5 / (RRF_K + 1.0) * (n.min(3) as f64));
            }
        }

        if cfg.tsk {
            // Anchors: the highest-scoring candidates so far. Python sorts the
            // score dict by -score, which is a stable sort over insertion
            // order; Scores reproduces that order.
            let mut items: Vec<(i64, f64)> = score.iter().collect();
            items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let anchors: Vec<i64> = items.iter().take(TSK_ANCHORS).map(|(v, _)| *v).collect();
            for (vid, n) in self.tsk_expand(&anchors, mode) {
                origins.entry(vid).or_default().insert("tsk");
                score.add(vid, 0.25 / (RRF_K + 1.0) * (n.min(4) as f64));
            }
        }

        let mut full: Vec<(i64, f64)> = Vec::with_capacity(score.len());
        full.extend(score.iter());
        full.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0))
        });

        let full_set: Vec<Candidate> = full
            .into_iter()
            .map(|(vid, s)| {
                let mut o: Vec<String> = origins
                    .get(&vid)
                    .map(|set| set.iter().map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                o.sort();
                Candidate {
                    verse_id: vid,
                    score: round_to(s, 6),
                    origins: o,
                    canon: self.index.canon_of_verse(vid).to_string(),
                }
            })
            .collect();
        let top: Vec<Candidate> = full_set.iter().take(top_n).cloned().collect();
        (full_set, top, matched_topics)
    }

    /// Group a scored candidate list into verse ranges within one chapter.
    pub fn as_ranges(&self, items: &[Candidate]) -> Vec<Passage> {
        let mut by_id: Vec<&Candidate> = items.iter().collect();
        by_id.sort_by_key(|c| c.verse_id);

        struct Cur {
            b: i64,
            c: i64,
            first: i64,
            last: i64,
            ids: Vec<i64>,
            origins: HashSet<String>,
            score: f64,
            canon: String,
        }
        let mut out: Vec<Cur> = Vec::new();
        let mut cur: Option<Cur> = None;
        for it in by_id {
            let vid = it.verse_id;
            let (b, c, v) = (verse_book(vid), verse_chapter(vid), verse_num(vid));
            let extend = match &cur {
                Some(k) => k.b == b && k.c == c && v - k.last <= 1,
                None => false,
            };
            if extend {
                let k = cur.as_mut().unwrap();
                k.last = v;
                k.ids.push(vid);
                for o in &it.origins {
                    k.origins.insert(o.clone());
                }
                if it.score > k.score {
                    k.score = it.score;
                }
            } else {
                if let Some(k) = cur.take() {
                    out.push(k);
                }
                cur = Some(Cur {
                    b,
                    c,
                    first: v,
                    last: v,
                    ids: vec![vid],
                    origins: it.origins.iter().cloned().collect(),
                    score: it.score,
                    canon: it.canon.clone(),
                });
            }
        }
        if let Some(k) = cur.take() {
            out.push(k);
        }

        let mut passages: Vec<Passage> = out
            .into_iter()
            .map(|k| {
                let a = self.index.abbrev(k.b);
                let n = self.index.name(k.b);
                let (reference, display_reference) = if k.first == k.last {
                    (format!("{} {}:{}", a, k.c, k.first), format!("{} {}:{}", n, k.c, k.first))
                } else {
                    (
                        format!("{} {}:{}-{}", a, k.c, k.first, k.last),
                        format!("{} {}:{}-{}", n, k.c, k.first, k.last),
                    )
                };
                let mut origins: Vec<String> = k.origins.into_iter().collect();
                origins.sort();
                Passage {
                    reference,
                    display_reference,
                    verse_ids: k.ids,
                    score: k.score,
                    origins,
                    canon: k.canon,
                }
            })
            .collect();
        // Python's list.sort is stable, so equal scores keep verse order.
        passages.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        passages
    }

    /// The cut sent to generation.
    ///
    /// In both-canon mode the Deuterocanon slice sits at the bottom of the
    /// ranked set by construction, so a prefix of the full list would return
    /// more protestant passages and never reach it. Two bugs of exactly that
    /// shape were found in P3.
    pub fn top_cut(
        &self,
        ranges: &[Passage],
        mode: CanonMode,
        top_n: usize,
        deut_n: usize,
    ) -> Vec<Passage> {
        if mode == CanonMode::Both {
            let mut out: Vec<Passage> =
                ranges.iter().filter(|p| p.canon == "protestant").take(top_n).cloned().collect();
            out.extend(ranges.iter().filter(|p| p.canon == "deutero").take(deut_n).cloned());
            out
        } else {
            ranges.iter().take(top_n).cloned().collect()
        }
    }
}

/// Python's round(x, n) for a positive number of digits: round half to even.
pub fn round_to(x: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    let y = x * f;
    let fract = y - y.trunc();
    let r = if fract.abs() == 0.5 {
        let down = y.trunc();
        if (down as i64) % 2 == 0 {
            down
        } else {
            down + y.signum()
        }
    } else {
        y.round()
    };
    r / f
}
