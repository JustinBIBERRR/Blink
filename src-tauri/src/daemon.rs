use crate::scheduler::Scheduler;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

pub type SharedState = Arc<Mutex<Scheduler>>;

/// Time advances only here. Presentation callbacks must re-read the scheduler
/// when they execute, not carry an old phase across an asynchronous boundary.
pub async fn run(
    shared: SharedState,
    on_tick: impl Fn(Option<crate::state::SessionOutcome>) + Send + 'static,
) {
    loop {
        sleep(Duration::from_millis(50)).await;
        let outcome = {
            let mut scheduler = shared.lock().unwrap();
            scheduler.is_silenced = crate::silence::SilenceChecker::new().is_silenced();
            let now = std::time::Instant::now();
            scheduler.refresh_pause(crate::pause_store::wall_now(), now);
            scheduler.tick(now)
        };
        on_tick(outcome);
    }
}
