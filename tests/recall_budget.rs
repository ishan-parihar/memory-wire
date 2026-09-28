//! The recall budget must be able to hold a median-size memory.
//!
//! `DEFAULT_RECALL_BUDGET` was 2,000 tokens while the median LongMemEval-S
//! session is 2,626 (4 chars/token, `docs/CONSISTENCY.md` §14.9). The trim is a
//! hard ceiling, so a budget under the median cannot return a median memory: the
//! top hit is cut to the cap and every lower hit is skipped. Retrieval metrics
//! could not see it — `eval/RESULTS.md`'s harness passes its own explicit
//! 100,000-token budget — which is exactly why this is pinned as an assertion
//! rather than a benchmark number.
use memory_wire::api::{MemoryService, DEFAULT_RECALL_BUDGET};
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store};

/// 4 chars per token, the store's own approximation (`recall::CHARS_PER_TOKEN`).
const CHARS_PER_TOKEN: usize = 4;
/// The median session, in tokens. §14.9.
const MEDIAN_SESSION_TOKENS: usize = 2_626;
const MEDIAN_SESSION_CHARS: usize = MEDIAN_SESSION_TOKENS * CHARS_PER_TOKEN;

/// A session-shaped memory of exactly `chars` characters, with `needle` early
/// enough to rank and `marker` near the end so a truncated copy is detectable.
fn session(needle: &str, chars: usize) -> String {
    let mut s = format!("{needle} ");
    s.push_str(&"filler conversation text. ".repeat((chars / 26) + 2));
    s.truncate(chars);
    s
}

fn svc() -> MemoryService<SqliteStore> {
    let store = SqliteStore::open_in_memory().expect("open");
    store.put_bank(&Bank { id: "b".into(), name: "b".into() }).expect("bank");
    MemoryService::new(store)
}

#[test]
fn a_median_size_session_must_survive_the_default_budget_whole() {
    let svc = svc();
    // One median-size session and one short distractor, so the recall has to
    // choose rather than return everything.
    let median = session("jose migration", MEDIAN_SESSION_CHARS);
    svc.retain("b", &median, None).expect("retain median");
    svc.retain("b", "unrelated note about the release checklist", None).expect("retain short");

    let hits = svc
        .recall_filtered("b", "jose migration", None, &[])
        .expect("recall under the shipped default budget");
    let served = hits.iter().find(|h| h.memory.content.starts_with("jose migration"));

    assert!(
        served.is_some(),
        "the median-size session was dropped at a {DEFAULT_RECALL_BUDGET}-token budget; \
         served: {:?}",
        hits.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(
        served.expect("present").memory.content.chars().count(),
        MEDIAN_SESSION_CHARS,
        "a median session must come back whole, not cut to the cap"
    );
}

#[test]
fn the_default_budget_must_admit_several_median_size_sessions_not_one() {
    let svc = svc();
    for i in 0..4 {
        svc.retain("b", &session(&format!("session{i} topic"), MEDIAN_SESSION_CHARS), None)
            .expect("retain");
    }
    let hits = svc
        .recall_filtered("b", "session1 topic", None, &[])
        .expect("recall under the shipped default budget");

    // 3 × 2,626 is the derivation of the default, so three whole sessions must
    // fit. The bug this pins is not "one is too few" but "the second one is
    // unreachable": at 2,000 tokens the first hit consumed the whole cap and
    // every later hit was skipped, so a consumer got a truncated first and
    // nothing else.
    assert!(
        hits.len() >= 3,
        "expected the default budget to admit at least 3 median sessions, got {}",
        hits.len()
    );
    let whole = hits
        .iter()
        .filter(|h| h.memory.content.chars().count() == MEDIAN_SESSION_CHARS)
        .count();
    assert!(
        whole >= 3,
        "expected at least 3 sessions served whole, got {whole} of {}",
        hits.len()
    );
}

/// The derivation itself, so the number and the reason cannot drift apart: the
/// default must be at least three median sessions.
#[test]
fn the_default_budget_must_be_at_least_three_median_sessions() {
    const _: () = assert!(
        DEFAULT_RECALL_BUDGET >= 3 * MEDIAN_SESSION_TOKENS,
        "DEFAULT_RECALL_BUDGET is below 3 × the median session in docs/CONSISTENCY.md §14.9"
    );
}
