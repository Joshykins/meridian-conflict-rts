//! Named timers and counters for finding what costs time, in the sim, the
//! renderer and the tools alike.
//!
//! Every call site owns a static slot; values live in thread-local totals that
//! only ever grow, so recording is a plain add (no atomics, no locks after the
//! first call). A [`Scope`] snapshots those totals and diffs them at its end,
//! which lets scopes nest (a headless frame around a sim tick) and keeps tests
//! that run on parallel threads from seeing each other's counts. Nothing here
//! feeds back into the simulation, so determinism is unaffected.
//!
//! ```ignore
//! let _t = mc_core::perf_span!("targeting.acquire"); // time + call count
//! mc_core::perf_count!("los.rays", rays);             // add to a counter
//! ```
//!
//! Names are dotted paths; reports treat the part before the first dot as the
//! group. Count in a local inside tight loops and add once after the loop.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// One call site's slot. Built by the macros; `id` is 0 until first use.
pub struct Site {
    pub name: &'static str,
    id: AtomicU32,
}

static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

impl Site {
    pub const fn new(name: &'static str) -> Site {
        Site {
            name,
            id: AtomicU32::new(0),
        }
    }

    #[inline]
    fn index(&self) -> usize {
        let id = self.id.load(Ordering::Relaxed);
        if id != 0 {
            return id as usize - 1;
        }
        self.register()
    }

    #[cold]
    fn register(&self) -> usize {
        let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        let id = self.id.load(Ordering::Relaxed);
        if id != 0 {
            return id as usize - 1;
        }
        // Two sites with the same name share one slot, so a counter can be
        // bumped from several places.
        let index = match names.iter().position(|n| *n == self.name) {
            Some(i) => i,
            None => {
                names.push(self.name);
                names.len() - 1
            }
        };
        self.id.store(index as u32 + 1, Ordering::Relaxed);
        index
    }
}

#[derive(Clone, Default)]
struct Totals {
    /// Calls for spans, the summed value for counters.
    n: Vec<u64>,
    /// Nanoseconds, spans only.
    ns: Vec<u64>,
    span: Vec<bool>,
}

impl Totals {
    #[inline]
    fn slot(&mut self, i: usize) {
        if i >= self.n.len() {
            self.n.resize(i + 1, 0);
            self.ns.resize(i + 1, 0);
            self.span.resize(i + 1, false);
        }
    }
}

thread_local! {
    static LOCAL: RefCell<Totals> = RefCell::new(Totals::default());
}

/// Adds `n` to a counter. Use [`perf_count!`](crate::perf_count) instead.
#[inline]
pub fn add(site: &'static Site, n: u64) {
    let i = site.index();
    LOCAL.with(|t| {
        let mut t = t.borrow_mut();
        t.slot(i);
        t.n[i] += n;
    });
}

/// Adds a finished span. Use [`perf_span!`](crate::perf_span) instead.
#[inline]
pub fn add_span(site: &'static Site, ns: u64) {
    let i = site.index();
    LOCAL.with(|t| {
        let mut t = t.borrow_mut();
        t.slot(i);
        t.n[i] += 1;
        t.ns[i] += ns;
        t.span[i] = true;
    });
}

/// Times the rest of the enclosing block.
pub struct SpanGuard {
    site: &'static Site,
    start: Instant,
}

impl SpanGuard {
    #[inline]
    pub fn new(site: &'static Site) -> SpanGuard {
        SpanGuard {
            site,
            start: Instant::now(),
        }
    }
}

impl Drop for SpanGuard {
    #[inline]
    fn drop(&mut self) {
        add_span(self.site, self.start.elapsed().as_nanos() as u64);
    }
}

/// `perf_span!("group.name")` times the rest of the block and counts calls.
#[macro_export]
macro_rules! perf_span {
    ($name:literal) => {{
        static SITE: $crate::perf::Site = $crate::perf::Site::new($name);
        $crate::perf::SpanGuard::new(&SITE)
    }};
}

/// `perf_count!("group.name")` adds one; `perf_count!("group.name", n)` adds `n`.
#[macro_export]
macro_rules! perf_count {
    ($name:literal) => {
        $crate::perf_count!($name, 1)
    };
    ($name:literal, $n:expr) => {{
        static SITE: $crate::perf::Site = $crate::perf::Site::new($name);
        $crate::perf::add(&SITE, ($n) as u64)
    }};
}

/// A `'static` copy of a name built at run time (GPU scope names with a
/// suffix, per-blueprint counters). Each distinct name is leaked once.
pub fn intern(name: &str) -> &'static str {
    static SEEN: Mutex<Option<std::collections::HashSet<&'static str>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let seen = seen.get_or_insert_with(Default::default);
    if let Some(s) = seen.get(name) {
        return s;
    }
    let s: &'static str = Box::leak(name.to_owned().into_boxed_str());
    seen.insert(s);
    s
}

/// Adds what another thread recorded to this thread's totals: how pool work
/// run on workers is counted as the caller's.
pub fn absorb(recorded: &Recorded) {
    LOCAL.with(|t| {
        let mut t = t.borrow_mut();
        for &(i, n, ns, span) in &recorded.entries {
            let i = i as usize;
            t.slot(i);
            t.n[i] += n;
            if span {
                t.ns[i] += ns;
                t.span[i] = true;
            }
        }
    });
}

/// What one thread recorded between [`Scope::begin`] and [`Scope::recorded`],
/// by site slot: cheap to take on a worker for every chunk of pool work, as it
/// looks up no names and takes no lock.
#[derive(Default)]
pub struct Recorded {
    entries: Vec<(u32, u64, u64, bool)>,
}

impl Recorded {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Records what this thread does between `begin` and `end`.
pub struct Scope {
    start: Totals,
}

impl Scope {
    pub fn begin() -> Scope {
        Scope {
            start: LOCAL.with(|t| t.borrow().clone()),
        }
    }

    /// What was recorded since `begin`, for [`absorb`] on another thread.
    pub fn recorded(self) -> Recorded {
        LOCAL.with(|t| {
            let now = t.borrow();
            let mut out = Recorded::default();
            for i in 0..now.n.len() {
                let n0 = self.start.n.get(i).copied().unwrap_or(0);
                let ns0 = self.start.ns.get(i).copied().unwrap_or(0);
                let (n, ns) = (now.n[i] - n0, now.ns[i] - ns0);
                if n != 0 || ns != 0 {
                    out.entries.push((i as u32, n, ns, now.span[i]));
                }
            }
            out
        })
    }

    /// What was recorded since `begin`, as a frame.
    pub fn end(self) -> Frame {
        let now = LOCAL.with(|t| t.borrow().clone());
        let names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        let mut frame = Frame::default();
        for i in 0..now.n.len() {
            let n0 = self.start.n.get(i).copied().unwrap_or(0);
            let ns0 = self.start.ns.get(i).copied().unwrap_or(0);
            let (n, ns) = (now.n[i] - n0, now.ns[i] - ns0);
            if n == 0 && ns == 0 {
                continue;
            }
            frame.entries.push(Entry {
                name: names[i],
                n,
                ns: now.span[i].then_some(ns),
            });
        }
        frame
    }
}

/// One named value of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: &'static str,
    /// Calls for a span, the value for a counter.
    pub n: u64,
    /// Time for a span; `None` for a counter.
    pub ns: Option<u64>,
}

/// Everything recorded over one tick or one rendered frame.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub entries: Vec<Entry>,
}

impl Frame {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// A counter's value, or a span's call count; 0 when not recorded.
    pub fn n(&self, name: &str) -> u64 {
        self.get(name).map_or(0, |e| e.n)
    }

    /// A span's time in nanoseconds; 0 when not recorded.
    pub fn ns(&self, name: &str) -> u64 {
        self.get(name).and_then(|e| e.ns).unwrap_or(0)
    }

    /// Adds a value measured elsewhere (a GPU pass, a phase timer).
    pub fn push(&mut self, name: &'static str, n: u64, ns: Option<u64>) {
        match self.entries.iter_mut().find(|e| e.name == name) {
            Some(e) => {
                e.n += n;
                if let Some(ns) = ns {
                    e.ns = Some(e.ns.unwrap_or(0) + ns);
                }
            }
            None => self.entries.push(Entry { name, n, ns }),
        }
    }

    pub fn merge(&mut self, other: &Frame) {
        for e in &other.entries {
            self.push(e.name, e.n, e.ns);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Stat {
    span: bool,
    frames: u32,
    n_sum: u64,
    n_max: u64,
    ns_sum: u64,
    ns_max: u64,
}

/// Frames gathered over a run, summarised per name.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub title: String,
    frames: u32,
    stats: BTreeMap<&'static str, Stat>,
    /// Total time of each frame (the `total` span when present), for spikes.
    totals: Vec<u64>,
    /// Free-form facts about the run (scene, camera, size).
    pub notes: Vec<(String, String)>,
}

impl Report {
    pub fn new(title: impl Into<String>) -> Report {
        Report {
            title: title.into(),
            ..Report::default()
        }
    }

    pub fn note(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.notes.push((key.into(), value.into()));
    }

    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Adds one frame. `total` names the span that stands for the whole frame.
    pub fn add(&mut self, frame: &Frame, total: &str) {
        self.frames += 1;
        for e in &frame.entries {
            let s = self.stats.entry(e.name).or_default();
            s.frames += 1;
            s.n_sum += e.n;
            s.n_max = s.n_max.max(e.n);
            if let Some(ns) = e.ns {
                s.span = true;
                s.ns_sum += ns;
                s.ns_max = s.ns_max.max(ns);
            }
        }
        self.totals.push(frame.ns(total));
    }

    /// Mean per frame of a counter (or span calls).
    pub fn mean_n(&self, name: &str) -> f64 {
        self.stats
            .get(name)
            .map_or(0.0, |s| s.n_sum as f64 / self.frames.max(1) as f64)
    }

    pub fn max_n(&self, name: &str) -> u64 {
        self.stats.get(name).map_or(0, |s| s.n_max)
    }

    /// Mean milliseconds per frame of a span.
    pub fn mean_ms(&self, name: &str) -> f64 {
        self.stats
            .get(name)
            .map_or(0.0, |s| s.ns_sum as f64 / 1e6 / self.frames.max(1) as f64)
    }

    pub fn max_ms(&self, name: &str) -> f64 {
        self.stats.get(name).map_or(0.0, |s| s.ns_max as f64 / 1e6)
    }

    /// Readable table: spans by mean time, then counters by name.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let f = self.frames.max(1) as f64;
        let _ = writeln!(out, "== perf: {} ({} frames)", self.title, self.frames);
        for (k, v) in &self.notes {
            let _ = writeln!(out, "   {k}: {v}");
        }
        if !self.totals.is_empty() {
            let mut t = self.totals.clone();
            t.sort_unstable();
            let pick = |q: f64| t[((t.len() - 1) as f64 * q) as usize] as f64 / 1e6;
            let _ = writeln!(
                out,
                "   frame ms  median {:.2}  p95 {:.2}  max {:.2}",
                pick(0.5),
                pick(0.95),
                pick(1.0)
            );
        }
        let mut spans: Vec<_> = self.stats.iter().filter(|(_, s)| s.span).collect();
        spans.sort_by(|a, b| b.1.ns_sum.cmp(&a.1.ns_sum).then(a.0.cmp(b.0)));
        let _ = writeln!(
            out,
            "{:<34} {:>9} {:>9} {:>10}",
            "span", "mean ms", "max ms", "calls/fr"
        );
        for (name, s) in spans {
            let _ = writeln!(
                out,
                "{:<34} {:>9.3} {:>9.3} {:>10.1}",
                name,
                s.ns_sum as f64 / 1e6 / f,
                s.ns_max as f64 / 1e6,
                s.n_sum as f64 / f
            );
        }
        let counters: Vec<_> = self.stats.iter().filter(|(_, s)| !s.span).collect();
        if !counters.is_empty() {
            let _ = writeln!(out, "{:<34} {:>12} {:>12}", "counter", "mean/fr", "max");
            for (name, s) in counters {
                let _ = writeln!(
                    out,
                    "{:<34} {:>12.1} {:>12}",
                    name,
                    s.n_sum as f64 / f,
                    s.n_max
                );
            }
        }
        out
    }

    /// Machine-readable form, for `mc-perf diff` and saved baselines.
    pub fn json(&self) -> String {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let mut out = String::new();
        let _ = write!(
            out,
            "{{\"title\":\"{}\",\"frames\":{},\"notes\":{{",
            esc(&self.title),
            self.frames
        );
        for (i, (k, v)) in self.notes.iter().enumerate() {
            let _ = write!(
                out,
                "{}\"{}\":\"{}\"",
                if i > 0 { "," } else { "" },
                esc(k),
                esc(v)
            );
        }
        out.push_str("},\"frame_ns\":[");
        for (i, t) in self.totals.iter().enumerate() {
            let _ = write!(out, "{}{}", if i > 0 { "," } else { "" }, t);
        }
        out.push_str("],\"stats\":{");
        for (i, (name, s)) in self.stats.iter().enumerate() {
            let _ = write!(
                out,
                "{}\"{}\":{{\"span\":{},\"frames\":{},\"n_sum\":{},\"n_max\":{},\"ns_sum\":{},\"ns_max\":{}}}",
                if i > 0 { "," } else { "" },
                esc(name),
                s.span,
                s.frames,
                s.n_sum,
                s.n_max,
                s.ns_sum,
                s.ns_max
            );
        }
        out.push_str("}}\n");
        out
    }

    /// Writes `<path>` as JSON and `<path>.txt` beside it when `path` ends in
    /// `.json`; otherwise just the text table.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if path.extension().is_some_and(|e| e == "json") {
            std::fs::write(path, self.json())?;
            std::fs::write(path.with_extension("txt"), self.text())
        } else {
            std::fs::write(path, self.text())
        }
    }
}

/// One row of a report read back from JSON, per frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Row {
    pub span: bool,
    /// Mean per frame: calls for a span, the value for a counter.
    pub mean_n: f64,
    pub max_n: u64,
    pub mean_ms: f64,
    pub max_ms: f64,
}

/// A saved report, read back for comparing.
#[derive(Clone, Debug, Default)]
pub struct Saved {
    pub title: String,
    pub frames: u32,
    pub notes: Vec<(String, String)>,
    pub frame_ms: Vec<f64>,
    pub rows: BTreeMap<String, Row>,
}

impl Saved {
    /// Reads what [`Report::json`] wrote.
    pub fn parse(text: &str) -> Result<Saved, String> {
        let mut p = json::Parser {
            s: text.as_bytes(),
            i: 0,
        };
        let v = p.value()?;
        let obj = v.obj().ok_or("report is not an object")?;
        let get = |k: &str| obj.iter().find(|(n, _)| n == k).map(|(_, v)| v);
        let frames = get("frames").and_then(|v| v.num()).unwrap_or(0.0) as u32;
        let f = frames.max(1) as f64;
        let mut saved = Saved {
            title: get("title")
                .and_then(|v| v.str())
                .unwrap_or_default()
                .to_owned(),
            frames,
            ..Saved::default()
        };
        if let Some(notes) = get("notes").and_then(|v| v.obj()) {
            for (k, v) in notes {
                saved
                    .notes
                    .push((k.clone(), v.str().unwrap_or_default().to_owned()));
            }
        }
        if let Some(json::Value::Arr(a)) = get("frame_ns") {
            saved.frame_ms = a.iter().filter_map(|v| v.num()).map(|n| n / 1e6).collect();
        }
        for (name, st) in get("stats").and_then(|v| v.obj()).ok_or("no stats")? {
            let st = st.obj().ok_or("stat is not an object")?;
            let n = |k: &str| {
                st.iter()
                    .find(|(n, _)| n == k)
                    .and_then(|(_, v)| v.num())
                    .unwrap_or(0.0)
            };
            let span = st
                .iter()
                .any(|(k, v)| k == "span" && matches!(v, json::Value::Bool(true)));
            saved.rows.insert(
                name.clone(),
                Row {
                    span,
                    mean_n: n("n_sum") / f,
                    max_n: n("n_max") as u64,
                    mean_ms: n("ns_sum") / 1e6 / f,
                    max_ms: n("ns_max") / 1e6,
                },
            );
        }
        Ok(saved)
    }

    pub fn load(path: &std::path::Path) -> Result<Saved, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Saved::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn median_ms(&self) -> f64 {
        let mut t = self.frame_ms.clone();
        t.sort_by(f64::total_cmp);
        t.get(t.len() / 2).copied().unwrap_or(0.0)
    }
}

/// What changed from `a` to `b`: spans by the time they gained or lost,
/// counters by how many times over they grew or shrank. `top` rows of each.
pub fn diff(a: &Saved, b: &Saved, top: usize) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "== diff: {}  ->  {}", a.title, b.title);
    let _ = writeln!(
        out,
        "   frames {} -> {}   median frame {:.2} -> {:.2} ms",
        a.frames,
        b.frames,
        a.median_ms(),
        b.median_ms()
    );
    let names: std::collections::BTreeSet<&String> = a.rows.keys().chain(b.rows.keys()).collect();
    let zero = Row::default();
    let mut spans = Vec::new();
    let mut counters = Vec::new();
    for name in names {
        let (ra, rb) = (
            a.rows.get(name).unwrap_or(&zero),
            b.rows.get(name).unwrap_or(&zero),
        );
        if ra.span || rb.span {
            spans.push((name, ra.mean_ms, rb.mean_ms));
        } else {
            counters.push((name, ra.mean_n, rb.mean_n));
        }
    }
    spans.sort_by(|x, y| (y.2 - y.1).abs().total_cmp(&(x.2 - x.1).abs()));
    let _ = writeln!(
        out,
        "{:<40} {:>9} {:>9} {:>9}",
        "span (mean ms)", "before", "after", "change"
    );
    for (name, x, y) in spans
        .iter()
        .take(top)
        .filter(|s| (s.2 - s.1).abs() >= 0.005)
    {
        let _ = writeln!(out, "{name:<40} {x:>9.3} {y:>9.3} {:>+9.3}", y - x);
    }
    let ratio = |x: f64, y: f64| (y.max(1.0) / x.max(1.0)).ln().abs();
    counters.sort_by(|p, q| ratio(q.1, q.2).total_cmp(&ratio(p.1, p.2)));
    let _ = writeln!(
        out,
        "{:<40} {:>11} {:>11} {:>8}",
        "counter (mean)", "before", "after", "times"
    );
    for (name, x, y) in counters.iter().take(top).filter(|c| ratio(c.1, c.2) > 0.05) {
        let _ = writeln!(
            out,
            "{name:<40} {x:>11.1} {y:>11.1} {:>7.2}x",
            y.max(1e-9) / x.max(1e-9)
        );
    }
    out
}

mod json {
    pub(super) enum Value {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Value>),
        Obj(Vec<(String, Value)>),
    }

    impl Value {
        pub(super) fn obj(&self) -> Option<&Vec<(String, Value)>> {
            match self {
                Value::Obj(o) => Some(o),
                _ => None,
            }
        }
        pub(super) fn num(&self) -> Option<f64> {
            match self {
                Value::Num(n) => Some(*n),
                _ => None,
            }
        }
        pub(super) fn str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                _ => None,
            }
        }
    }

    pub(super) struct Parser<'a> {
        pub s: &'a [u8],
        pub i: usize,
    }

    impl Parser<'_> {
        fn ws(&mut self) {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
        }
        fn eat(&mut self, c: u8) -> Result<(), String> {
            self.ws();
            if self.s.get(self.i) == Some(&c) {
                self.i += 1;
                Ok(())
            } else {
                Err(format!("expected '{}' at byte {}", c as char, self.i))
            }
        }
        fn string(&mut self) -> Result<String, String> {
            self.eat(b'"')?;
            let mut out = Vec::new();
            while let Some(&c) = self.s.get(self.i) {
                self.i += 1;
                match c {
                    b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                    b'\\' => {
                        let e = *self.s.get(self.i).ok_or("bad escape")?;
                        self.i += 1;
                        out.push(match e {
                            b'n' => b'\n',
                            b't' => b'\t',
                            other => other,
                        });
                    }
                    _ => out.push(c),
                }
            }
            Err("unterminated string".into())
        }
        pub(super) fn value(&mut self) -> Result<Value, String> {
            self.ws();
            match self.s.get(self.i).copied() {
                Some(b'{') => {
                    self.i += 1;
                    let mut o = Vec::new();
                    self.ws();
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Value::Obj(o));
                    }
                    loop {
                        let k = self.string()?;
                        self.eat(b':')?;
                        o.push((k, self.value()?));
                        self.ws();
                        match self.s.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Value::Obj(o));
                            }
                            _ => return Err(format!("bad object at byte {}", self.i)),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = Vec::new();
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Value::Arr(a));
                    }
                    loop {
                        a.push(self.value()?);
                        self.ws();
                        match self.s.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Value::Arr(a));
                            }
                            _ => return Err(format!("bad array at byte {}", self.i)),
                        }
                    }
                }
                Some(b'"') => Ok(Value::Str(self.string()?)),
                Some(b't') if self.s[self.i..].starts_with(b"true") => {
                    self.i += 4;
                    Ok(Value::Bool(true))
                }
                Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                    self.i += 5;
                    Ok(Value::Bool(false))
                }
                Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                    self.i += 4;
                    Ok(Value::Null)
                }
                _ => {
                    let start = self.i;
                    while self.i < self.s.len()
                        && matches!(
                            self.s[self.i],
                            b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                        )
                    {
                        self.i += 1;
                    }
                    std::str::from_utf8(&self.s[start..self.i])
                        .ok()
                        .and_then(|t| t.parse().ok())
                        .map(Value::Num)
                        .ok_or_else(|| format!("bad value at byte {start}"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_nest_and_diff() {
        let outer = Scope::begin();
        crate::perf_count!("test.a", 2);
        let inner = Scope::begin();
        crate::perf_count!("test.a", 3);
        {
            let _s = crate::perf_span!("test.span");
        }
        let i = inner.end();
        let o = outer.end();
        assert_eq!(i.n("test.a"), 3);
        assert_eq!(o.n("test.a"), 5);
        assert_eq!(i.n("test.span"), 1);
        assert!(i.get("test.span").unwrap().ns.is_some());
    }

    #[test]
    fn report_summarises() {
        let mut r = Report::new("t");
        let mut f = Frame::default();
        f.push("tick", 1, Some(2_000_000));
        f.push("hits", 10, None);
        r.add(&f, "tick");
        f.entries[1].n = 30;
        r.add(&f, "tick");
        assert_eq!(r.mean_n("hits"), 20.0);
        assert_eq!(r.max_n("hits"), 30);
        assert!((r.mean_ms("tick") - 2.0).abs() < 1e-9);
        assert!(r.text().contains("hits"));
        assert!(r.json().starts_with("{\"title\":\"t\""));
    }

    #[test]
    fn json_round_trips_and_diffs() {
        let mut r = Report::new("before \"q\"");
        r.note("camera", "1,2,3");
        let mut f = Frame::default();
        f.push("gpu.scene", 1, Some(4_000_000));
        f.push("spatial.tested", 100, None);
        r.add(&f, "gpu.scene");
        let a = Saved::parse(&r.json()).unwrap();
        assert_eq!(a.title, "before \"q\"");
        assert_eq!(a.rows["spatial.tested"].mean_n, 100.0);
        assert!((a.rows["gpu.scene"].mean_ms - 4.0).abs() < 1e-9);
        let mut b = a.clone();
        b.rows.get_mut("spatial.tested").unwrap().mean_n = 500.0;
        let d = diff(&a, &b, 10);
        assert!(d.contains("spatial.tested") && d.contains("5.00x"), "{d}");
    }
}
