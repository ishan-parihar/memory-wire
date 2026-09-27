//! Phase 4 regression gate: retain → recall → reflect + isolation + redaction.
use memory_wire::api::MemoryService;
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store};

fn bank(id: &str) -> Bank {
    Bank {
        id: id.into(),
        name: id.into(),
    }
}

#[test]
fn e2e_should_retain_recall_and_reflect_with_isolation() {
    let store = SqliteStore::open_in_memory().expect("open");
    store.put_bank(&bank("proj")).expect("bank");
    store.put_bank(&bank("other")).expect("bank");
    let svc = MemoryService::new(store);
    svc.retain("proj", "auth uses jose in src/middleware/auth.ts", None)
        .expect("retain");
    svc.retain("proj", "rate limiting via token bucket", None)
        .expect("retain");
    let hits = svc.recall("proj", "jose auth", 2000).expect("recall");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].memory.content.contains("jose"));
    assert!(svc.recall("other", "jose", 2000).expect("recall").is_empty());
    let answer = svc.reflect("proj", "how does auth work", &[]).expect("reflect");
    assert!(answer.contains("jose"));
}

#[test]
fn e2e_should_redact_secrets_end_to_end() {
    let store = SqliteStore::open_in_memory().expect("open");
    store.put_bank(&bank("b")).expect("bank");
    let svc = MemoryService::new(store);
    let id = svc.retain("b", "deploy key sk-abcDEF123456", None).expect("retain");
    let got = svc.store.get("b", &id).expect("get").expect("some");
    assert!(got.content.contains("[REDACTED:api_key]"));
}
