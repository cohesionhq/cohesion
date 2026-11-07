use std::time::Duration;

use cohesion_core::model::ContextRebuildReason;
use jiff::Timestamp;

use super::{Fitness, ThreadMark, fitness_of};
use crate::session::LastCall;
use crate::system_prompt::HeardMemories;

#[test]
fn a_thread_rebuilds_once_its_cache_expires_or_its_prompt_grows_too_big() {
    let sent_at = Timestamp::now();
    let call = |prompt_tokens| LastCall {
        sent_at,
        prompt_tokens,
    };
    let ttl = Duration::from_secs(5 * 60);
    let soon = sent_at + Duration::from_secs(4 * 60);
    let later = sent_at + Duration::from_secs(6 * 60);

    assert_eq!(fitness_of(call(50_000), ttl, soon), Fitness::CarryOn);
    assert_eq!(
        fitness_of(call(50_000), ttl, later),
        Fitness::Rebuild(ContextRebuildReason::CacheExpired)
    );

    assert_eq!(fitness_of(call(100_000), ttl, soon), Fitness::CarryOn);
    assert_eq!(
        fitness_of(call(100_001), ttl, soon),
        Fitness::Rebuild(ContextRebuildReason::ContextLimit)
    );
}

// However long since its latest call, a thread is read back until its cache
// expires, then rebuilds.
#[test]
fn a_thread_carries_on_until_its_cache_expires_then_rebuilds() {
    let sent_at = Timestamp::now();
    let call = LastCall {
        sent_at,
        prompt_tokens: 50_000,
    };
    let hours = |hours: u64| sent_at + Duration::from_secs(hours * 60 * 60);
    let mark = ThreadMark {
        thread_id: 1,
        items: 2,
        model_id: 7,
        memory_limit: 20,
        extended_from: 1,
        heard_through: 1,
        heard: HeardMemories::default(),
        last_call: call,
        cache_ttl: Duration::from_secs(12 * 60 * 60),
    };

    assert_eq!(mark.fitness(7, 20, hours(11)), Fitness::CarryOn);

    assert_eq!(
        mark.fitness(7, 20, hours(13)),
        Fitness::Rebuild(ContextRebuildReason::CacheExpired)
    );

    assert_eq!(
        mark.fitness(8, 20, hours(1)),
        Fitness::Rebuild(ContextRebuildReason::ModelChanged)
    );

    assert_eq!(
        mark.fitness(7, 21, hours(1)),
        Fitness::Rebuild(ContextRebuildReason::MemoryCapacityChanged)
    );
}
