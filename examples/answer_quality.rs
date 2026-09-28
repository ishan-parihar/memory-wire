//! LongMemEval-S **answer-quality** harness: LLM-judged answer accuracy, plus the
//! closed-book control that says whether retrieval earned any of it.
//!
//! The project measures `recall_any@K` — did the gold session land in the top K.
//! That is a proxy for the thing anyone actually cares about: whether an agent that
//! reads what `recall` returned ends up with the right answer. It is possible to
//! retrieve the correct session, bury it at position 14 of 20, score full `R@5`, and
//! still get the answer wrong. Nothing in `eval/` can see that, and this harness is
//! what sees it.
//!
//! ## Protocol
//!
//! Two LLM calls per question, in the order the LongMemEval paper specifies and the
//! released evaluator implements (`xiaowu0162/longmemeval`, `eval_utils.py`): the
//! model **generates** an answer from the retrieved evidence, then a **judge** grades
//! that answer against the gold answer. Both arms run the same sequence:
//!
//! | arm | context |
//! |---|---|
//! | **retrieval-conditioned** | the top-k memories `MemoryService::recall` actually returns, at the shipped default budget |
//! | **closed-book control** | none — same questions, same model, same judge |
//!
//! **The closed-book arm is the point.** Without it a retrieval-conditioned accuracy
//! proves nothing: the model may simply know the answer, and a system that retrieves
//! perfectly can score the same as one that retrieves nothing. If the delta is near
//! zero, that is a finding about this project's retrieval work, and the harness
//! reports it as prominently as any other number rather than burying it.
//!
//! ## What this number is not
//!
//! It is **not** comparable to a published LoCoMo or LongMemEval figure, and it is
//! **not** `recall_any@K`. The judge here is whatever `MEMORY_WIRE_LLM_JUDGE_MODEL`
//! names, reached over an OpenAI-compatible chat endpoint with the prompt embedded in
//! the artifact below — not the official grader, not the paper's judge model, and not
//! run over the official evidence formatting. A different judge, prompt, model or
//! truncation rule is a different instrument. The numbers are this harness's, labelled
//! as such.
//!
//! PII redaction is **not** exercised: the haystack is indexed through `Store::put`,
//! exactly as `examples/longmemeval.rs` does, so this measures the ranking and the
//! answer loop, not the redacted write path.
//!
//! ## Configuration
//!
//! No provider is hardcoded. Required:
//! `MEMORY_WIRE_LLM_URL`, `MEMORY_WIRE_LLM_MODEL`, `MEMORY_WIRE_LLM_KEY`.
//! Optional: `MEMORY_WIRE_LLM_JUDGE_MODEL` (defaults to the answering model),
//! `MEMORY_WIRE_LLM_TEMPERATURE` (default 0), `MEMORY_WIRE_LLM_MAX_TOKENS`
//! (default 256). Unconfigured is a **hard failure with instructions** — never a
//! silent zero, because a zero that looks like a score is how an eval harness starts
//! lying.
//!
//! ## Cost
//!
//! Four LLM calls per question, so a 500-question pass is 2,000 calls. The default
//! slice is `--n 25` (100 calls) under a seeded shuffle, so it is a real sample and
//! reproducible. A full run is an explicit `--n 500`.
//!
//! Run: `cargo run --release --example answer_quality -- --data eval/data/longmemeval_s_cleaned.json --n 25`

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use memory_wire::api::{MemoryService, DEFAULT_RECALL_BUDGET};
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};
use serde::Deserialize;

// The `--out-md` policy (a bare run must not be able to overwrite a committed
// `eval/` artifact) and the build-profile name the provenance line needs.
mod bench_common;

/// Default slice. 4 calls/question, so this is 100 LLM calls — a real sample of the
/// suite, not a full pass. A 500-question run costs 2,000 calls and has to be asked
/// for by name.
const DEFAULT_N: usize = 25;

/// LLM calls per question: answer (conditioned), answer (closed-book), judge x2.
const CALLS_PER_QUESTION: usize = 4;

const ENV_URL: &str = "MEMORY_WIRE_LLM_URL";
const ENV_MODEL: &str = "MEMORY_WIRE_LLM_MODEL";
const ENV_KEY: &str = "MEMORY_WIRE_LLM_KEY";
const ENV_JUDGE_MODEL: &str = "MEMORY_WIRE_LLM_JUDGE_MODEL";
const ENV_TEMPERATURE: &str = "MEMORY_WIRE_LLM_TEMPERATURE";
const ENV_MAX_TOKENS: &str = "MEMORY_WIRE_LLM_MAX_TOKENS";

#[derive(Deserialize)]
struct Turn {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct Entry {
    question_id: String,
    question_type: String,
    question: String,
    /// The gold answer, judged against. Present in `longmemeval_s_cleaned.json`,
    /// but **typed inconsistently there**: 468 of the 500 entries carry a string and
    /// 32 carry a bare integer (`"answer": 3`). Read as a JSON value and rendered by
    /// [`gold_text`], because a harness that refuses 6% of the suite and refuses
    /// loudly is worse than one that reads them.
    ///
    /// `examples/longmemeval.rs` never deserializes this field — it only measures
    /// recall — which is why this has never been a problem there.
    answer: serde_json::Value,
    answer_session_ids: Vec<String>,
    haystack_session_ids: Vec<String>,
    haystack_sessions: Vec<Vec<Turn>>,
}

// ---------------------------------------------------------------- LLM plumbing

/// The gold answer as prompt text: a JSON string verbatim, anything else in its JSON
/// form. Rendering the value rather than requiring a string is what lets the 32
/// numeric answers in the export through instead of aborting the run.
fn gold_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Clone, Copy, Default, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

struct Completion {
    text: String,
    usage: Usage,
}

struct Llm {
    url: String,
    answer_model: String,
    judge_model: String,
    key: Option<String>,
    temperature: f64,
    max_tokens: usize,
}

impl Llm {
    /// Read the endpoint, the models and the key from the environment, or refuse to
    /// start.
    ///
    /// The refusal is the whole design: a harness that answers an unconfigured run
    /// with `0.0%` is worse than no harness, because the number looks like a
    /// measurement of the system when it is a measurement of nothing.
    fn from_env() -> Result<Self> {
        let url = std::env::var(ENV_URL).ok().unwrap_or_default();
        let answer_model = std::env::var(ENV_MODEL).ok().unwrap_or_default();
        let mut missing: Vec<&str> = Vec::new();
        if url.trim().is_empty() {
            missing.push(ENV_URL);
        }
        if answer_model.trim().is_empty() {
            missing.push(ENV_MODEL);
        }
        if !missing.is_empty() {
            anyhow::bail!(
                "answer_quality: LLM access is not configured, so there is no number to \
                 report.\nMissing: {missing}\n\n\
                 Required; any OpenAI-compatible chat-completions endpoint works:\n  \
                 {URL}\n    base URL, e.g. http://localhost:8081/v1\n  \
                 {MODEL}\n    model id for the answering call\n  \
                 {KEY}\n    bearer token; some local gateways ignore it\n\n\
                 Optional:\n  \
                 {JUDGE}\n    judge model id, defaults to {MODEL}\n  \
                 {TEMP}\n    sampling temperature, default 0\n  \
                 {TOK}\n    answer token cap, default 256\n\n\
                 Every question costs {CALLS} LLM calls, so the default slice of --n \
                 {DEFAULT_N} is already {SMALL} calls. Do not start a run without \
                 intending to pay for it.",
                missing = missing.join(", "),
                URL = ENV_URL,
                MODEL = ENV_MODEL,
                KEY = ENV_KEY,
                JUDGE = ENV_JUDGE_MODEL,
                TEMP = ENV_TEMPERATURE,
                TOK = ENV_MAX_TOKENS,
                CALLS = CALLS_PER_QUESTION,
                SMALL = DEFAULT_N * CALLS_PER_QUESTION,
            );
        }
        let judge_model = std::env::var(ENV_JUDGE_MODEL)
            .ok()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| answer_model.clone());
        let temperature = std::env::var(ENV_TEMPERATURE)
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0);
        let max_tokens = std::env::var(ENV_MAX_TOKENS)
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(256);
        Ok(Self {
            url,
            answer_model,
            judge_model,
            key: std::env::var(ENV_KEY).ok().filter(|k| !k.trim().is_empty()),
            temperature,
            max_tokens,
        })
    }

    /// One throwaway call, so a run pointed at a dead or misconfigured endpoint
    /// fails in seconds instead of after `n * 4` identical failures and a table of
    /// zeros at the end. Costs one call; saves a whole run.
    fn preflight(&self, client: &reqwest::blocking::Client) -> Result<()> {
        self.chat(client, &self.answer_model, "Reply with exactly: OK")
            .map(|_| ())
    }

    /// One chat completion. Every failure path names the endpoint, the model and the
    /// offending body — a bare "request failed" here is indistinguishable, six
    /// questions later, from a 0% score in the artifact.
    fn chat(
        &self,
        client: &reqwest::blocking::Client,
        model: &str,
        prompt: &str,
    ) -> Result<Completion> {
        let endpoint = format!("{}/chat/completions", self.url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
        });
        let mut req = client.post(&endpoint).json(&body);
        if let Some(key) = &self.key {
            req = req.bearer_auth(key);
        }
        let response = req
            .send()
            .with_context(|| format!("POST {endpoint} failed"))?;
        let status = response.status();
        let text = response
            .text()
            .with_context(|| format!("could not read the body of POST {endpoint}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "{endpoint} ({model}) returned {status}: {}",
                clip(&text, 400)
            );
        }
        let parsed: ChatResponse = serde_json::from_str(&text).with_context(|| {
            format!(
                "{endpoint} ({model}) returned a body this harness cannot parse: {}",
                clip(&text, 400)
            )
        })?;
        let text = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();
        if text.trim().is_empty() {
            anyhow::bail!("{endpoint} ({model}) returned an empty completion");
        }
        Ok(Completion {
            text,
            usage: parsed.usage.unwrap_or_default(),
        })
    }
}

// ------------------------------------------------------------------ the prompts

/// The answering prompt, used by **both** arms.
///
/// The closed-book control is the same function with an empty context list, so the
/// two arms differ in exactly one input — the retrieved memories — and nothing else.
/// Two prompt templates would confound the control with wording.
///
/// `I don't know` is the only permitted non-answer, so a question whose evidence is
/// absent produces a stated refusal rather than a plausible invention; that is what
/// makes the closed-book arm meaningful instead of a second chance to guess.
fn answer_prompt(question: &str, contexts: &[String]) -> String {
    let memories = if contexts.is_empty() {
        "(none - no memories were retrieved for this question)".to_string()
    } else {
        contexts
            .iter()
            .enumerate()
            .map(|(i, c)| format!("[memory {i}]\n{c}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    format!(
        "You are answering a question about a person using only the numbered memories \
         below.\n\
         \n\
         Memories:\n{memories}\n\
         \n\
         Question: {question}\n\
         \n\
         Answer with the shortest span that answers the question. If the memories do not \
         contain the answer, reply exactly: I don't know. Do not use outside knowledge."
    )
}

/// The judge prompt — the published LongMemEval shape (reference answer, candidate
/// answer, one verdict) with a machine-readable verdict so no rubric parser is
/// needed.
///
/// **This is not the official grader prompt.** It differs from the released evaluator
/// in its framing sentence, in spelling out the equivalence rule, and in asking for
/// JSON rather than a `yes`/`no` token; and it runs against whatever
/// `MEMORY_WIRE_LLM_JUDGE_MODEL` names rather than the paper's judge model. Those
/// differences are why the resulting accuracy is this harness's own number and not a
/// reproduction of a published one. The prompt is embedded verbatim in the artifact so
/// a reader can audit exactly what was asked.
fn judge_prompt(question: &str, gold: &str, predicted: &str) -> String {
    format!(
        "You are grading one candidate answer against the reference answer for a question.\n\
         \n\
         Question:\n{question}\n\
         \n\
         <|The Start of Reference Answer|>\n{gold}\n<|The End of Reference Answer|>\n\
         \n\
         <|The Start of Candidate Answer|>\n{predicted}\n<|The End of Candidate Answer|>\n\
         \n\
         The candidate is correct if it conveys the same information as the reference. \
         Ignore wording, formatting, and extra detail that does not contradict the \
         reference. A candidate that omits the key fact, or answers a different question, \
         is incorrect. If the reference says the information is not known or not stated, \
         then a candidate that says so is correct.\n\
         \n\
         Reply with one JSON object and nothing else:\n\
         {{\"correct\": true or false, \"reason\": \"one short sentence\"}}"
    )
}

/// Pull the verdict out of a judge reply.
///
/// Models fence JSON, prefix it with `Sure!`, and trail it with a sentence, so the
/// whole body is tried first and the outermost brace pair second. A reply with no
/// object in it is an **error**, not a false verdict: scoring a malformed judge
/// response as "incorrect" would quietly move the number the harness exists to report.
fn parse_verdict(reply: &str) -> Result<(bool, String)> {
    let trimmed = reply.trim();
    let candidates = [
        trimmed.to_string(),
        match (trimmed.find('{'), trimmed.rfind('}')) {
            (Some(open), Some(close)) if close > open => trimmed[open..=close].to_string(),
            _ => String::new(),
        },
    ];
    for candidate in &candidates {
        if candidate.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
            if let Some(correct) = v.get("correct").and_then(serde_json::Value::as_bool) {
                let reason = v
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                return Ok((correct, reason));
            }
        }
    }
    anyhow::bail!("no JSON verdict in the judge reply: {}", clip(trimmed, 400))
}

// ----------------------------------------------------------------- the harness

/// One arm's tally. `scored` counts only questions that produced a verdict: a
/// question whose LLM call failed has no answer, and counting it as a wrong answer
/// would make a broken run look like a bad system.
#[derive(Default, Clone, Copy)]
struct Arm {
    scored: usize,
    correct: usize,
    prompt_tokens: u64,
    total_tokens: u64,
}

impl Arm {
    fn pct(&self) -> f64 {
        ratio(self.correct, self.scored) * 100.0
    }

    /// The accuracy to print. `n/a`, never `0.0%`, when nothing was scored.
    ///
    /// This was not a hypothetical. A run against a wedged endpoint produced a bold
    /// `**0.0%**` in the artifact's headline table, correct in its denominator and
    /// wrong in everything a skimming reader takes from it. A zero with no
    /// denominator is the one number this harness must never invent, so the empty
    /// case refuses to render a percentage at all.
    fn label(&self) -> String {
        match self.scored {
            0 => "n/a".to_string(),
            _ => format!("{:.1}%", self.pct()),
        }
    }

    fn add(&mut self, verdict: Option<bool>, usage: Usage) {
        if let Some(correct) = verdict {
            self.scored += 1;
            self.correct += usize::from(correct);
        }
        self.prompt_tokens += usage.prompt_tokens;
        self.total_tokens += usage.total_tokens;
    }
}

/// Per-`question_type` tally. The three counters are the disagreement buckets: how
/// often the gold session was in the served context, how often that produced a wrong
/// answer, and how often an absent gold still got a correct one.
#[derive(Default)]
struct TypeAgg {
    conditioned: Arm,
    closed_book: Arm,
    in_context: usize,
    in_context_but_wrong: usize,
    absent_but_right: usize,
}

/// `--out-json`, or a scratch path under `$TMPDIR`.
///
/// `bench_common::out_md` is the shared definition of this rule for markdown, and it
/// announces the markdown flag; reusing it verbatim here would print an instruction
/// about `--out-md` while writing a JSON file. So the same rule is restated for the
/// JSON name. The reason it is restated at all: the sibling `longmemeval` harness
/// defaults `--out-json` to `eval/results.json`, so a bare run of *that* binary
/// overwrites a committed artifact. This one must not.
fn out_json(args: &[String], name: &str) -> String {
    match bench_common::flag(args, "--out-json") {
        Some(explicit) => explicit,
        None => {
            let path = std::env::temp_dir().join(name);
            eprintln!(
                "no --out-json given: writing the scratch artifact to {} (pass `--out-json \
                 eval/{name}` to update the committed one)",
                path.display()
            );
            path.display().to_string()
        }
    }
}

fn arg_num<T>(name: &str, default: T, args: &[String]) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match bench_common::flag(args, name) {
        None => Ok(default),
        Some(v) => v
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("{name} wants a number, got {v:?}: {e}")),
    }
}

/// `2500` -> `2,500`, matching `examples/longmemeval.rs`. The number is read by
/// humans, and a missing separator is the kind of thing a later copy-paste "fixes"
/// into something else.
fn group_digits(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// `/proc/loadavg` at one moment, or `unavailable`. `AGENTS.md` §3 makes every timing
/// number conditional on the load it was taken under.
fn loadavg() -> String {
    fs::read_to_string("/proc/loadavg").map_or_else(
        |_| "unavailable (no /proc/loadavg)".to_string(),
        |s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" / "),
    )
}

/// `git rev-parse HEAD`, so the artifact names the tree it measured.
fn commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown (not a git worktree)".to_string())
}

/// Whether the tree was clean when the run started, and which paths were not.
///
/// A commit SHA alone is a lie when the worktree has uncommitted edits: the SHA
/// names a tree that is not the code that ran. An artifact that cannot tell the two
/// apart is not reproducible, so the paths go in the artifact next to the SHA.
fn tree_state() -> String {
    let out = match Command::new("git").args(["status", "--porcelain"]).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return "unknown (git status failed)".to_string(),
    };
    let paths: Vec<String> = out
        .lines()
        .map(|l| l.get(3..).unwrap_or(l).trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if paths.is_empty() {
        return "clean".to_string();
    }
    format!(
        "dirty - {} uncommitted path(s): {}",
        paths.len(),
        clip(&paths.join(", "), 300)
    )
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}… ({} chars total)", s.chars().count())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data = bench_common::arg(&args, "--data", "eval/data/longmemeval_s_cleaned.json");
    let n = arg_num("--n", DEFAULT_N, &args)?;
    let seed = arg_num("--seed", 42u64, &args)?;
    let budget = arg_num("--budget", DEFAULT_RECALL_BUDGET, &args)?;
    let out_json = out_json(&args, "ANSWER_QUALITY.json");
    let out_md = bench_common::out_md(&args, "ANSWER_QUALITY.md");
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: this run is not `--release`; the wall-clock figures below are not \
             comparable to the committed artifacts"
        );
    }
    if n == 0 {
        anyhow::bail!("--n 0 scores nothing; pass a positive count (default {DEFAULT_N})");
    }

    // Before the 264 MB dataset is read, and before any token is spent: with no LLM
    // configured this run cannot produce a number, and must say so rather than emit a
    // zero that would read as a score.
    let llm = Llm::from_env()?;
    let calls = n * CALLS_PER_QUESTION;
    eprintln!(
        "answer_quality: {n} questions, seed {seed}, recall budget {budget} tokens, \
         {calls} LLM calls\nanswering with {}, judging with {}",
        llm.answer_model, llm.judge_model
    );
    if n > DEFAULT_N {
        eprintln!(
            "WARNING: --n {n} is above the default {DEFAULT_N}; that is a full-cost pass. \
             The default slice exists because this harness spends real money."
        );
    }

    let load_before = loadavg();
    let raw =
        fs::read_to_string(&data).with_context(|| format!("reading the dataset at {data}"))?;
    let entries: Vec<Entry> =
        serde_json::from_str(&raw).context("parsing the dataset (a LongMemEval-S export)")?;
    let mut order = bench_common::shuffle(entries.len(), seed);
    if n < order.len() {
        order.truncate(n);
    }
    eprintln!("loaded {} entries, scoring {}", entries.len(), order.len());

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .context("building the HTTP client")?;
    llm.preflight(&client).map_err(|err| {
        anyhow::anyhow!(
            "the configured endpoint did not answer a one-call probe, so this run would \
             score nothing: {err:#}\n\
             Fix the endpoint or the key and re-run. The probe costs one call; the run it \
             just saved costs {calls}."
        )
    })?;

    let started = Instant::now();
    let mut conditioned = Arm::default();
    let mut closed_book = Arm::default();
    let mut errors: Vec<String> = Vec::new();
    let mut rows: Vec<serde_json::Value> = Vec::new();
    let mut by_type: HashMap<String, TypeAgg> = HashMap::new();

    for (qi, &ei) in order.iter().enumerate() {
        let e = &entries[ei];
        // Per-question fresh index, session-as-document — the indexing method
        // `examples/longmemeval.rs` uses, so the context served here is the context
        // that harness measured recall over.
        let store = SqliteStore::open_in_memory().context("opening the scratch index")?;
        store.put_bank(&Bank {
            id: "eval".into(),
            name: "eval".into(),
        })?;
        for (sid, turns) in e
            .haystack_session_ids
            .iter()
            .zip(e.haystack_sessions.iter())
        {
            let text: Vec<String> = turns
                .iter()
                .map(|t| format!("{}: {}", t.role, t.content))
                .collect();
            store.put(&Memory {
                id: sid.clone(),
                bank_id: "eval".into(),
                content: text.join("\n"),
                context: None,
                created_at: None,
            })?;
        }
        let svc = MemoryService::new(store);
        let hits = svc
            .recall("eval", &e.question, budget)
            .with_context(|| format!("recall for question {}", e.question_id))?;
        // The default `recall` surface serves bare content, so that is what the
        // answerer gets: no ids, no scores, nothing an agent would not have had.
        let contexts: Vec<String> = hits.iter().map(|h| h.memory.content.clone()).collect();
        let first_gold = hits
            .iter()
            .position(|h| e.answer_session_ids.contains(&h.memory.id));
        let in_context = first_gold.is_some();

        let ask = |question_contexts: &[String]| match llm.chat(
            &client,
            &llm.answer_model,
            &answer_prompt(&e.question, question_contexts),
        ) {
            Ok(c) => (Some(c.text), c.usage, None),
            Err(err) => (None, Usage::default(), Some(err.to_string())),
        };
        let (answer_c, usage_c, err_c) = ask(&contexts);
        let (answer_b, usage_b, err_b) = ask(&[]);

        let gold = gold_text(&e.answer);
        // One grading function, both arms: same judge, same prompt shape, so a gap
        // between the two columns is the contexts and nothing else.
        let judge = |prediction: &Option<String>| -> (Option<bool>, String, Option<String>, Usage) {
            let Some(prediction) = prediction else {
                return (
                    None,
                    String::new(),
                    Some("no answer to grade".to_string()),
                    Usage::default(),
                );
            };
            match llm.chat(
                &client,
                &llm.judge_model,
                &judge_prompt(&e.question, &gold, prediction),
            ) {
                Ok(c) => match parse_verdict(&c.text) {
                    Ok((correct, reason)) => (Some(correct), reason, None, c.usage),
                    Err(err) => (None, String::new(), Some(err.to_string()), c.usage),
                },
                Err(err) => (None, String::new(), Some(err.to_string()), Usage::default()),
            }
        };
        let (verdict_c, reason_c, jerr_c, judge_c) = judge(&answer_c);
        let (verdict_b, reason_b, jerr_b, judge_b) = judge(&answer_b);

        // The judge's own tokens are real spend, so they land in both arms' token
        // counters (each arm had exactly one graded answer) and the reported total
        // equals the real total. The verdicts stay in their own columns.
        conditioned.add(verdict_c, sum(usage_c, judge_c));
        closed_book.add(verdict_b, sum(usage_b, judge_b));
        let bucket = by_type.entry(e.question_type.clone()).or_default();
        bucket.conditioned.add(verdict_c, Usage::default());
        bucket.closed_book.add(verdict_b, Usage::default());
        bucket.in_context += usize::from(in_context);
        bucket.in_context_but_wrong += usize::from(in_context && verdict_c == Some(false));
        bucket.absent_but_right += usize::from(!in_context && verdict_c == Some(true));

        let row_errors: Vec<String> = [err_c, jerr_c, err_b, jerr_b]
            .into_iter()
            .flatten()
            .collect();
        for err in &row_errors {
            // `{err:#}` is anyhow's one-line cause chain. `{}` prints only the
            // outermost context, which for every network failure is the same
            // "POST ... failed" and never says why — timeout, refused, DNS.
            errors.push(format!("{}: {err:#}", e.question_id));
        }

        rows.push(serde_json::json!({
            "question_id": e.question_id,
            "question_type": e.question_type,
            "question": e.question,
            "gold": gold,
            "n_contexts": contexts.len(),
            "first_gold_rank": first_gold,
            "gold_in_context": in_context,
            "answer_conditioned": answer_c,
            "verdict_conditioned": verdict_c,
            "judge_reason_conditioned": reason_c,
            "answer_closed_book": answer_b,
            "verdict_closed_book": verdict_b,
            "judge_reason_closed_book": reason_b,
            "errors": row_errors,
        }));

        if (qi + 1) % 5 == 0 || qi + 1 == order.len() {
            eprintln!(
                "  {done}/{total} — conditioned {acc_c:.1}% (n={s_c}), closed-book {acc_b:.1}% \
                 (n={s_b})",
                done = qi + 1,
                total = order.len(),
                acc_c = conditioned.pct(),
                s_c = conditioned.scored,
                acc_b = closed_book.pct(),
                s_b = closed_book.scored,
            );
        }
    }
    let elapsed = started.elapsed();
    let load_after = loadavg();

    // ----------------------------------------------------------------- report
    let attempted = order.len();
    let delta = conditioned.pct() - closed_book.pct();
    let in_context_total: usize = by_type.values().map(|t| t.in_context).sum();
    let unearned: usize = by_type.values().map(|t| t.in_context_but_wrong).sum();
    let rewarded = in_context_total.saturating_sub(unearned);
    let flukes: usize = by_type.values().map(|t| t.absent_but_right).sum();
    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let profile_token = if bench_common::profile() == "release" {
        "release"
    } else {
        "debug"
    };
    // A delta over an empty denominator is not a small delta, it is no delta.
    let delta_label = match (conditioned.scored, closed_book.scored) {
        (0, _) | (_, 0) => "n/a".to_string(),
        _ => format!("{:+.1}pp", delta),
    };
    let mut types: Vec<&String> = by_type.keys().collect();
    types.sort();

    let mut md = format!(
        "# LongMemEval-S answer quality (memory-wire)\n\
         \n\
         ## This number is not a benchmark score\n\
         \n\
         **It is not comparable to any published LoCoMo or LongMemEval figure, and it \
         is not a competitor score.** LongMemEval's published metric is LLM-judged *answer* \
         accuracy over a two-stage protocol: generate an answer from the retrieved \
         evidence, then grade it against the gold answer with an LLM judge. This harness \
         reproduces the shape of that protocol against memory-wire's own shipped \
         retrieval, but the judge here is `{judge}`, reached over an OpenAI-compatible \
         chat endpoint, with the prompt printed below — not the official grader, not the \
         paper's judge model, and not run over the official evidence formatting. A \
         different judge, prompt, model or truncation rule is a different instrument, so \
         the accuracy below belongs to this harness alone.\n\
         \n\
         **It is also not `recall_any@K`.** `eval/RESULTS.md` asks whether the gold \
         session landed in the top K. This asks whether an LLM, handed what `recall` \
         actually returned, produced an answer a grader accepted. The two can diverge in \
         both directions, and the disagreement table below is why this harness exists.\n\
         \n\
         PII redaction is not exercised: the haystack is indexed through `Store::put`, \
         as `examples/longmemeval.rs` does, so this measures the ranking and the answer \
         loop, not the redacted write path.\n\
         \n\
         ## Provenance\n\
         \n\
         | field | value |\n|---|---|\n\
         | commit | `{commit}` |\n\
         | tree state | {tree_state} |\n\
         | date | {date} |\n\
         | command | `{command}` |\n\
         | profile | `--{profile_token}` |\n\
         | host | {host} |\n\
         | dataset | `{data}` |\n\
         | slice | {attempted} questions, seed {seed} (LCG shuffle, as \
         `examples/longmemeval.rs`) |\n\
         | recall budget | {budget} tokens (`DEFAULT_RECALL_BUDGET` when not passed) |\n\
         | answering model | `{answer_model}` |\n\
         | judge model | `{judge}` |\n\
         | temperature | {temperature} |\n\
         | answer token cap | {max_tokens} |\n\
         | api key | {key_state} |\n\
         | loadavg before | {load_before} |\n\
         | loadavg after | {load_after} |\n\
         | wall clock | {secs:.1} s over {calls} LLM calls |\n\
         \n\
         Load average is recorded because `AGENTS.md` §3 makes any timing number \
         conditional on it. Note also what is *not* being timed: these calls are \
         dominated by provider and network latency, so the retrieval the harness also \
         performs is nowhere near the wall clock below.\n\
         \n\
         ## Result\n\
         \n\
         | arm | accuracy | correct | scored | unscored | prompt tokens | total tokens |\n\
         |---|---|---|---|---|---|---|\n\
         | retrieval-conditioned | **{acc_c}** | {c_c} | {s_c} | {e_c} | {pt_c} | {tt_c} |\n\
         | closed-book control | **{acc_b}** | {c_b} | {s_b} | {e_b} | {pt_b} | {tt_b} |\n\
         | **delta (conditioned - closed-book)** | **{delta}** | | | | | |\n\
         \n\
         Denominators are **scored** questions, not attempted: a question whose LLM call \
         failed has no verdict and is excluded rather than counted wrong, and `unscored` \
         says how many were excluded. **A cell reading `n/a` means nothing was scored at \
         all** — the run produced no number, and no accuracy below may be quoted from it. \
         A non-zero `unscored` on a cell that does have a percentage makes that percentage \
         incomparable to a clean run: treat the whole artifact as provisional.\n",
        judge = llm.judge_model,
        commit = commit(),
        tree_state = tree_state(),
        command = clip(&command, 400),
        host = bench_common::host(),
        attempted = attempted,
        answer_model = llm.answer_model,
        temperature = llm.temperature,
        max_tokens = llm.max_tokens,
        key_state = if llm.key.is_some() {
            "set (value not recorded here)"
        } else {
            "unset"
        },
        load_before = load_before,
        load_after = load_after,
        secs = elapsed.as_secs_f64(),
        calls = attempted * CALLS_PER_QUESTION,
        acc_c = conditioned.label(),
        c_c = conditioned.correct,
        s_c = conditioned.scored,
        e_c = attempted.saturating_sub(conditioned.scored),
        pt_c = group_digits(conditioned.prompt_tokens as usize),
        tt_c = group_digits(conditioned.total_tokens as usize),
        acc_b = closed_book.label(),
        c_b = closed_book.correct,
        s_b = closed_book.scored,
        e_b = attempted.saturating_sub(closed_book.scored),
        pt_b = group_digits(closed_book.prompt_tokens as usize),
        tt_b = group_digits(closed_book.total_tokens as usize),
        delta = delta_label,
    );

    md.push_str(&format!(
        "\n## Where recall and answer disagree\n\
         \n\
         The failure this harness exists to catch: the gold session is retrieved, so \
         `R@K` scores full marks, and the answer is still wrong. That is the third line.\n\
         \n\
         | on this slice | count | share of n={attempted} |\n|---|---|---|\n\
         | gold session present in the served context | {in_context_total} | {in_c_pct:.1}% |\n\
         | … and the answer was judged correct (recall rewarded) | {rewarded} | {rew_pct:.1}% |\n\
         | … and the answer was judged **wrong** (recall not rewarded) | {unearned} | {un_pct:.1}% |\n\
         | gold session absent, answer judged correct anyway | {flukes} | {fl_pct:.1}% |\n\
         \n\
         The third line is an answerer problem, not a retrieval problem: the evidence was \
         served and the answer did not use it. The fourth is the closed-book floor, and it \
         bounds how much of the first two lines is real memory use at all. A third line far \
         below the second, together with a delta near zero, would say that the ranking work \
         in `eval/RESULTS.md` is not buying a better answer — which is a result to record, \
         not a bug in this harness.\n",
        in_context_total = in_context_total,
        in_c_pct = ratio(in_context_total, attempted) * 100.0,
        rewarded = rewarded,
        rew_pct = ratio(rewarded, attempted) * 100.0,
        unearned = unearned,
        un_pct = ratio(unearned, attempted) * 100.0,
        flukes = flukes,
        fl_pct = ratio(flukes, attempted) * 100.0,
    ));

    md.push_str(
        "\n## Per question type\n\n\
         | type | conditioned | closed-book | delta | gold in context | scored |\n\
         |---|---|---|---|---|---|\n",
    );
    for t in &types {
        let agg = &by_type[*t];
        let type_delta = match (agg.conditioned.scored, agg.closed_book.scored) {
            (0, _) | (_, 0) => "n/a".to_string(),
            _ => format!("{:+.1}pp", agg.conditioned.pct() - agg.closed_book.pct()),
        };
        md.push_str(&format!(
            "| {t} | {} | {} | {type_delta} | {} | {} |\n",
            agg.conditioned.label(),
            agg.closed_book.label(),
            agg.in_context,
            agg.conditioned.scored,
        ));
    }

    md.push_str(&format!(
        "\n## The judge prompt, verbatim\n\
         \n\
         What the judge was asked, once per question per arm. Reproduced here so the \
         number above can be audited rather than trusted.\n\
         \n```text\n{judge}\n```\n\
         \n\
         ## The answering prompt, verbatim\n\
         \n\
         Identical for both arms. The closed-book control is this same function with an \
         empty context list, which is what makes the two arms differ in exactly one \
         input.\n\
         \n```text\n{answer}\n```\n\
         \n\
         ## Errors\n\n",
        judge = judge_prompt("<question>", "<gold answer>", "<candidate answer>"),
        answer = answer_prompt(
            "<question>",
            &["<memory 0, as recall served it>".to_string()]
        ),
    ));
    if errors.is_empty() {
        md.push_str("None: every question was answered and judged.\n");
    } else {
        md.push_str(&format!(
            "{count} of {attempted} questions lost at least one call, listed per question:\n\
             \n```text\n{detail}\n```\n",
            count = errors.len(),
            detail = errors.join("\n"),
        ));
    }

    md.push_str(&format!(
        "\n## Reproduce\n\
         \n\
         ```bash\n\
         export {URL}='<openai-compatible base url>'\n\
         export {MODEL}='<answering model id>'\n\
         export {KEY}='<bearer token>'\n\
         # optional: export {JUDGE}='<judge model id>'\n\
         cargo run --release --example answer_quality -- \\\n  \
         --data {data} --n {attempted} --seed {seed}\n\
         ```\n\
         \n\
         `--out-md` and `--out-json` write under `$TMPDIR` unless named, so a bare run \
         cannot overwrite a committed `eval/` artifact. Updating one means naming it, the \
         same rule as every other harness in this repo.\n",
        URL = ENV_URL,
        MODEL = ENV_MODEL,
        KEY = ENV_KEY,
        JUDGE = ENV_JUDGE_MODEL,
    ));

    fs::write(&out_json, serde_json::to_string(&rows)?)
        .with_context(|| format!("writing {out_json}"))?;
    fs::write(&out_md, &md).with_context(|| format!("writing {out_md}"))?;

    eprintln!(
        "\nretrieval-conditioned {acc_c} (n={s_c}) | closed-book {acc_b} (n={s_b}) | \
         delta {delta}\n{total_prompt} prompt tokens, {total_all} total tokens, {calls} LLM \
         calls, {secs:.1}s wall\nloadavg {load_before} -> {load_after}; {errs} row errors\nwrote \
         {out_json} + {out_md}",
        acc_c = conditioned.label(),
        s_c = conditioned.scored,
        acc_b = closed_book.label(),
        s_b = closed_book.scored,
        delta = delta_label,
        total_prompt =
            group_digits((conditioned.prompt_tokens + closed_book.prompt_tokens) as usize),
        total_all = group_digits((conditioned.total_tokens + closed_book.total_tokens) as usize),
        calls = attempted * CALLS_PER_QUESTION,
        secs = elapsed.as_secs_f64(),
        load_before = load_before,
        load_after = load_after,
        errs = errors.len(),
    );
    Ok(())
}

/// Token counts of two calls, added.
fn sum(a: Usage, b: Usage) -> Usage {
    Usage {
        prompt_tokens: a.prompt_tokens + b.prompt_tokens,
        completion_tokens: a.completion_tokens + b.completion_tokens,
        total_tokens: a.total_tokens + b.total_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_parses_bare_json() {
        let (correct, reason) =
            parse_verdict(r#"{"correct": true, "reason": "same span"}"#).unwrap();
        assert!(correct);
        assert_eq!(reason, "same span");
    }

    #[test]
    fn verdict_parses_fenced_json_behind_a_preamble() {
        let reply =
            "Sure! Here it is:\n```json\n{\"correct\": false, \"reason\": \"wrong name\"}\n```\n";
        let (correct, reason) = parse_verdict(reply).unwrap();
        assert!(!correct);
        assert_eq!(reason, "wrong name");
    }

    /// A malformed judge reply must be an error, not a wrong answer: counting it as
    /// `false` would move the very number the harness exists to report.
    #[test]
    fn verdict_without_json_is_an_error_not_a_false() {
        assert!(parse_verdict("the candidate looks right to me").is_err());
    }

    /// One prompt, two arms: the closed-book control must be the conditioned prompt
    /// with the memory block emptied, or the delta measures wording too.
    #[test]
    fn closed_book_prompt_is_the_same_prompt_without_memories() {
        let book = answer_prompt("q?", &[]);
        let served = answer_prompt("q?", &["a".to_string(), "b".to_string()]);
        assert!(book.contains("no memories were retrieved"));
        assert!(!book.contains("[memory 0]"));
        assert!(served.contains("[memory 0]\na"));
        assert!(served.contains("[memory 1]\nb"));
        assert_eq!(
            book.replace("(none - no memories were retrieved for this question)", ""),
            served.replace("[memory 0]\na\n\n[memory 1]\nb", "")
        );
    }

    /// 32 of the 500 `answer` fields in `longmemeval_s_cleaned.json` are bare
    /// integers, so a string-typed field would abort the run on a sixth of the suite.
    #[test]
    fn gold_renders_strings_and_numbers() {
        assert_eq!(
            gold_text(&serde_json::json!("Business Admin")),
            "Business Admin"
        );
        assert_eq!(gold_text(&serde_json::json!(3)), "3");
        assert_eq!(gold_text(&serde_json::json!(3.5)), "3.5");
        assert_eq!(gold_text(&serde_json::json!(null)), "null");
    }

    #[test]
    fn accuracy_divides_by_scored_questions_not_attempted_ones() {
        let mut arm = Arm::default();
        arm.add(Some(true), Usage::default());
        arm.add(Some(false), Usage::default());
        arm.add(None, Usage::default());
        assert_eq!(arm.scored, 2);
        assert_eq!(arm.correct, 1);
        assert_eq!(arm.pct(), 50.0);
        assert_eq!(Arm::default().pct(), 0.0);
    }

    #[test]
    fn loadavg_is_a_triple_or_says_so() {
        let l = loadavg();
        assert!(
            l == "unavailable (no /proc/loadavg)" || l.split(" / ").count() == 3,
            "unexpected loadavg string: {l}"
        );
    }

    #[test]
    fn clip_marks_what_it_cut() {
        assert_eq!(clip("short", 10), "short");
        assert!(clip("abcdef", 3).starts_with("abc"));
    }
}
