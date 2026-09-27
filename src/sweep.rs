//! `sweep`: forget what a bank said it would stop remembering.
//!
//! Explicit invocation, always — there is no scheduler and no thread. Forgetting
//! is the one destructive thing in the crate, so it happens when somebody asks
//! for it, and a bank with no `ttl_days` keeps every memory it was ever given.

use chrono::{DateTime, TimeDelta, Utc};
use memory_wire::store::{Store, StoreError};

/// The store's `created_at` shape, which is what makes the age comparison a byte
/// compare: fixed-width RFC 3339 UTC with exactly three fractional digits, the
/// same thing `strftime('%Y-%m-%dT%H:%M:%fZ', …)` writes.
const TIMESTAMP: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";

/// Why a bank was not touched, when it was not touched.
const NO_TTL: &str = "no ttl_days";
/// Why a bank was not touched, when it named a retention window that cannot be
/// expressed as a date.
const TTL_OUT_OF_RANGE: &str = "ttl_days is further out than a date can hold";

/// What one bank did, or would have done, to its own memories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankSweep {
    /// Bank id.
    pub bank: String,
    /// Retention window, in days.
    pub ttl_days: u32,
    /// Oldest `created_at` still kept: a memory created exactly at this stamp
    /// survives, because it is not *older* than the cutoff.
    pub cutoff: String,
    /// Memories deleted, or — under `--dry-run` — that would be.
    pub removed: usize,
}

/// One bank the sweep declined to touch, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    /// Bank id.
    pub bank: String,
    /// Why nothing was deleted.
    pub reason: &'static str,
}

/// What a whole sweep did, bank by bank.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Banks that had a `ttl_days`, in the order the store listed them.
    pub swept: Vec<BankSweep>,
    /// Banks that had none. Named, and never touched.
    pub skipped: Vec<Skip>,
    /// Sum of the per-bank removals.
    pub total: usize,
}

impl Report {
    /// The screen.
    ///
    /// `dry_run` picks the verb, so a report can never claim a deletion that did
    /// not happen.
    pub fn render(&self, dry_run: bool) -> String {
        let (verb, tally) = if dry_run {
            ("would delete", "would be deleted")
        } else {
            ("deleted", "deleted")
        };
        let mut out = String::from("memory-wire sweep");
        for row in &self.swept {
            out.push_str(&format!(
                "\n  {:<10}  ttl {}d  cutoff {}  {} {}",
                row.bank, row.ttl_days, row.cutoff, verb, row.removed
            ));
        }
        for skip in &self.skipped {
            out.push_str(&format!("\n  {:<10}  skipped ({})", skip.bank, skip.reason));
        }
        out.push_str(&format!("\ntotal {} {tally}", self.total));
        if self.swept.is_empty() {
            out.push_str("\nnothing was ever a candidate: no bank sets ttl_days");
        }
        out
    }
}

/// The oldest `created_at` a bank with `ttl_days` keeps, as of `now`.
///
/// `None` when the window reaches further out than a date can be represented.
/// A `u32` of days is a span of millions of years and the calendar is nowhere
/// near that wide, so a checked subtraction is the only correct answer here: a
/// window that far out is indistinguishable from never, and the caller skips the
/// bank rather than panicking on a number somebody typed into a config.
pub fn cutoff(ttl_days: u32, now: DateTime<Utc>) -> Option<String> {
    let ttl = TimeDelta::try_days(i64::from(ttl_days))?;
    Some(now.checked_sub_signed(ttl)?.format(TIMESTAMP).to_string())
}

/// Sweep every bank, expiring what its own policy says is too old.
pub fn run<S: Store>(store: &S, dry_run: bool) -> Result<Report, StoreError> {
    run_at(store, dry_run, Utc::now())
}

/// [`run`] with the clock supplied, so the age boundary is a value a test
/// chooses rather than one it races.
pub fn run_at<S: Store>(
    store: &S,
    dry_run: bool,
    now: DateTime<Utc>,
) -> Result<Report, StoreError> {
    let mut report = Report::default();
    for (bank, ttl_days) in store.bank_ttls()? {
        let Some(days) = ttl_days else {
            report.skipped.push(Skip { bank, reason: NO_TTL });
            continue;
        };
        let Some(cutoff) = cutoff(days, now) else {
            report.skipped.push(Skip { bank, reason: TTL_OUT_OF_RANGE });
            continue;
        };
        let removed = store.expire_before(&bank, &cutoff, dry_run)?;
        report.total += removed;
        report.swept.push(BankSweep {
            bank,
            ttl_days: days,
            cutoff,
            removed,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_wire::memory::Bank;
    use memory_wire::store::SqliteStore;

    /// A fixed instant, so "30 days ago" is a literal rather than a race.
    fn now() -> DateTime<Utc> {
        "2026-09-27T12:00:00Z".parse().expect("instant")
    }

    fn store() -> SqliteStore {
        SqliteStore::open_in_memory().expect("open")
    }

    fn bank(s: &SqliteStore, id: &str, ttl: Option<&str>) {
        s.put_bank(&Bank {
            id: id.to_string(),
            name: id.to_string(),
        })
        .expect("bank");
        let config = match ttl {
            Some(days) => format!(r#"{{"ttl_days":{days}}}"#),
            None => "{}".to_string(),
        };
        s.set_bank_config(id, &config).expect("config");
    }

    /// A memory with a pinned `created_at`; the store stamps one only when the
    /// caller leaves it `None`.
    fn aged(s: &SqliteStore, bank: &str, id: &str, content: &str, created_at: &str) {
        s.put(&memory_wire::memory::Memory {
            id: id.to_string(),
            bank_id: bank.to_string(),
            content: content.to_string(),
            context: None,
            created_at: Some(created_at.to_string()),
        })
        .expect("put");
    }

    // The cutoff has to land on the same wire the store writes, or the age
    // comparison silently compares different formats.
    #[test]
    fn cutoff_should_be_the_stores_own_timestamp_shape() {
        let got = cutoff(30, now()).expect("cutoff");
        assert_eq!(got, "2026-08-28T12:00:00.000Z", "{got}");
        assert_eq!(got.len(), "2026-08-28T12:00:00.000Z".len());
        // A window nothing can be older than still produces a stamp.
        assert_eq!(cutoff(0, now()).expect("zero"), "2026-09-27T12:00:00.000Z");
        // A window past what a date can hold is a skip, never a panic.
        assert_eq!(cutoff(u32::MAX, now()), None);
    }

    // A bank that asked to be forgotten loses exactly its expired rows, and the
    // rest of the store — including another bank's identical content — is
    // untouched. Ids are unique across banks because `memories.id` is the
    // primary key, not `(bank_id, id)`: a reused id relocates the row.
    #[test]
    fn a_bank_with_a_ttl_should_expire_and_its_neighbours_should_not() {
        let s = store();
        bank(&s, "aging", Some("30"));
        bank(&s, "permanent", None);
        aged(&s, "aging", "aging-stale", "jose middleware note", "2026-08-01T00:00:00.000Z");
        aged(&s, "aging", "aging-fresh", "jose deploy note", "2026-09-20T00:00:00.000Z");
        aged(&s, "permanent", "perm-stale", "jose middleware note", "2020-01-01T00:00:00.000Z");

        let report = run_at(&s, false, now()).expect("sweep");
        assert_eq!(report.total, 1, "{report:?}");
        assert_eq!(report.swept.len(), 1);
        assert_eq!(report.swept[0].bank, "aging");
        assert_eq!(report.swept[0].removed, 1);
        assert_eq!(report.swept[0].cutoff, "2026-08-28T12:00:00.000Z");

        // The skipped bank is named, and nothing of its own went.
        assert_eq!(report.skipped.len(), 1, "{report:?}");
        assert_eq!(report.skipped[0].bank, "permanent");
        assert_eq!(report.skipped[0].reason, NO_TTL);
        assert_eq!(s.list("aging").expect("list").len(), 1);
        assert_eq!(s.list("permanent").expect("list").len(), 1);
        // And the index followed the deletion, so recall cannot cite it.
        let hits = s.keyword_search("aging", "jose", 10).expect("search");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].0, "aging-fresh");
    }

    // `--dry-run` is the same arithmetic with the delete left out, so it must
    // report the count it would have produced and change nothing at all.
    #[test]
    fn a_dry_run_should_report_the_count_and_delete_nothing() {
        let s = store();
        bank(&s, "aging", Some("30"));
        aged(&s, "aging", "a", "jose one", "2020-01-01T00:00:00.000Z");
        aged(&s, "aging", "b", "jose two", "2021-01-01T00:00:00.000Z");
        aged(&s, "aging", "c", "jose three", "2026-09-26T00:00:00.000Z");

        let report = run_at(&s, true, now()).expect("dry run");
        assert_eq!(report.total, 2, "{report:?}");
        assert_eq!(s.list("aging").expect("list").len(), 3, "a dry run must delete nothing");
        assert_eq!(s.keyword_search("aging", "jose", 10).expect("search").len(), 3);
        assert!(
            report.render(true).contains("would delete 2"),
            "{}",
            report.render(true)
        );

        // The same invocation without --dry-run is what actually forgets them.
        assert_eq!(run_at(&s, false, now()).expect("sweep").total, 2);
        assert_eq!(s.list("aging").expect("list").len(), 1);
    }

    // The boundary itself: a memory created exactly at the cutoff is not older
    // than the cutoff, so it is kept, and one a second earlier is not.
    #[test]
    fn the_cutoff_boundary_should_keep_the_row_created_exactly_on_it() {
        let s = store();
        bank(&s, "edge", Some("30"));
        let cutoff = cutoff(30, now()).expect("cutoff");
        let just_before = "2026-08-28T11:59:59.999Z";
        assert_ne!(just_before, cutoff);
        aged(&s, "edge", "on", "jose exactly on the boundary", &cutoff);
        aged(&s, "edge", "before", "jose a moment before it", just_before);
        aged(&s, "edge", "after", "jose a moment after it", "2026-08-28T12:00:00.001Z");

        assert_eq!(run_at(&s, false, now()).expect("sweep").total, 1);
        let left: Vec<String> = s
            .list("edge")
            .expect("list")
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(left, ["on", "after"], "only the strictly-older row goes");
    }

    // A store nobody configured has nothing to do, and says so rather than
    // printing a bare zero that reads like a bank it failed to sweep.
    #[test]
    fn a_store_with_no_ttl_anywhere_should_report_no_candidates() {
        let s = store();
        bank(&s, "a", None);
        bank(&s, "b", None);
        let report = run_at(&s, false, now()).expect("sweep");
        assert_eq!(report.total, 0);
        assert!(report.swept.is_empty());
        assert_eq!(report.skipped.len(), 2);
        let screen = report.render(false);
        assert!(screen.contains("no bank sets ttl_days"), "{screen}");
        assert!(screen.contains("a"), "{screen}");
        assert!(screen.contains("b"), "{screen}");
    }
}
