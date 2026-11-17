//! Host-only lifecycle for a future explicit Linux score-bound offline export.
//! No IPC paths, compiler, renderer, scheduling, or serialized readiness here.
//! The host must supply the injected window owner and hold one registry mutex for
//! every operation, including `publish`. Never use a renderer-provided owner.
use std::{collections::BTreeMap, sync::{Arc, atomic::{AtomicBool, Ordering}}};

#[derive(Debug, PartialEq, Eq)]
pub enum Error { Busy, Stale, WrongPhase, Exhausted }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase { Pending, Ready, Rendering }

/// Not a bearer capability: every call also checks the injected window owner.
#[derive(Debug, Clone)]
pub struct Work {
    generation: u64,
    cancel: Arc<AtomicBool>,
}
impl Work {
    pub fn cancellation(&self) -> &AtomicBool { &self.cancel }
}
struct Slot<T> {
    work: Work,
    revision: String,
    source: String,
    phase: Phase,
    ready: Option<T>,
}
/// T should be a host-only pair of Audition and DryReadiness, never IPC JSON.
/// A registry outlives individual workers and must be cleared on reload/close.
pub struct Leases<T> {
    next: u64,
    windows: BTreeMap<String, Slot<T>>,
}
impl<T> Default for Leases<T> {
    fn default() -> Self { Self { next: 0, windows: BTreeMap::new() } }
}
impl<T> Leases<T> {
    pub fn begin(&mut self, owner: &str, revision: &str, source: &str) -> Result<Work, Error> {
        if self.windows.contains_key(owner) { return Err(Error::Busy); }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        let work = Work { generation: self.next, cancel: Arc::new(AtomicBool::new(false)) };
        self.windows.insert(owner.into(), Slot {
            work: work.clone(), revision: revision.into(), source: source.into(),
            phase: Phase::Pending, ready: None,
        });
        Ok(work)
    }
    fn matching(&self, owner: &str, work: &Work) -> bool {
        self.windows.get(owner).is_some_and(|slot|
            slot.work.generation == work.generation && Arc::ptr_eq(&slot.work.cancel, &work.cancel))
    }
    fn check(&self, owner: &str, work: &Work) -> Result<(), Error> {
        if self.matching(owner, work) && !work.cancel.load(Ordering::Acquire) { Ok(()) }
        else { Err(Error::Stale) }
    }
    pub fn phase(&self, owner: &str, work: &Work) -> Result<Phase, Error> {
        self.check(owner, work)?;
        Ok(self.windows[owner].phase)
    }
    /// Call on every accepted edit/revision, not just before a chooser or render.
    /// A changed revision invalidates even if the text was subsequently restored.
    pub fn observe_snapshot(&mut self, owner: &str, revision: &str, source: &str) {
        if self.windows.get(owner).is_some_and(|slot|
            slot.revision != revision || slot.source != source) { self.close(owner); }
    }
    /// Use for cancel/failure/finally. An old worker cannot cancel a newer export.
    pub fn cancel(&mut self, owner: &str, work: &Work) {
        if self.matching(owner, work) { self.close(owner); }
    }
    /// Window teardown/reload must call this even while a native chooser is open.
    pub fn close(&mut self, owner: &str) {
        if let Some(slot) = self.windows.remove(owner) {
            slot.work.cancel.store(true, Ordering::Release);
        }
    }
    /// Install a completed native preflight only while its original lease lives.
    /// On any error the incoming readiness is dropped, never retained elsewhere.
    pub fn ready(&mut self, owner: &str, work: &Work, ready: T) -> Result<(), Error> {
        self.check(owner, work)?;
        let slot = self.windows.get_mut(owner).unwrap();
        if slot.phase != Phase::Pending { return Err(Error::WrongPhase); }
        slot.ready = Some(ready);
        slot.phase = Phase::Ready;
        Ok(())
    }
    /// Transfer the non-serializable session to one worker, keeping its live lease.
    pub fn start_render(&mut self, owner: &str, work: &Work) -> Result<T, Error> {
        self.check(owner, work)?;
        let slot = self.windows.get_mut(owner).unwrap();
        if slot.phase != Phase::Ready { return Err(Error::WrongPhase); }
        slot.phase = Phase::Rendering;
        Ok(slot.ready.take().unwrap())
    }
    /// Final publication gate. Hold the same host registry mutex used by edits
    /// and cancellation. The closure may ONLY atomically publish an already
    /// validated temporary WAV; do not render, await, or call back into this lock.
    /// Return value preserves the writer's error. Both outcomes retire the lease.
    /// Use audio render_prepared_snapshot_with_gate; its PendingWav::commit is
    /// the short closure. Tauri revision/chooser wiring remains a separate step.
    pub fn publish<R>(&mut self, owner: &str, work: &Work, publish: impl FnOnce() -> R) -> Result<R, Error> {
        self.check(owner, work)?;
        if self.windows[owner].phase != Phase::Rendering { return Err(Error::WrongPhase); }
        // Remove first: even a panicking publisher cannot leave a reusable lease.
        let slot = self.windows.remove(owner).unwrap();
        struct Retire(Arc<AtomicBool>);
        impl Drop for Retire { fn drop(&mut self) { self.0.store(true, Ordering::Release); } }
        let _retire = Retire(slot.work.cancel);
        Ok(publish())
    }
}
impl<T> Drop for Leases<T> {
    fn drop(&mut self) {
        for slot in self.windows.values() { slot.work.cancel.store(true, Ordering::Release); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_ready_render_publish_exactly_once() {
        let mut s = Leases::default();
        let w = s.begin("main", "r1", "score").unwrap();
        assert_eq!(s.phase("main", &w), Ok(Phase::Pending));
        assert_eq!(s.start_render("main", &w), Err(Error::WrongPhase));
        s.ready("main", &w, 7).unwrap();
        assert_eq!(s.phase("main", &w), Ok(Phase::Ready));
        assert_eq!(s.ready("main", &w, 8), Err(Error::WrongPhase));
        assert_eq!(s.start_render("main", &w), Ok(7));
        assert_eq!(s.start_render("main", &w), Err(Error::WrongPhase));
        assert_eq!(s.publish("main", &w, || 42), Ok(42));
        assert_eq!(s.publish("main", &w, || panic!("stale publish")), Err(Error::Stale));
        assert!(w.cancellation().load(Ordering::Acquire));
    }
    #[test]
    fn cross_window_and_cross_registry_tokens_never_authorize() {
        let mut a = Leases::<()>::default(); let mut b = Leases::<()>::default();
        let w = a.begin("main", "1", "s").unwrap();
        let other = a.begin("other", "1", "s").unwrap();
        let foreign = b.begin("main", "1", "s").unwrap();
        assert_eq!(a.ready("other", &w, ()), Err(Error::Stale));
        assert_eq!(a.ready("main", &foreign, ()), Err(Error::Stale));
        a.cancel("main", &other);
        assert_eq!(a.phase("main", &w), Ok(Phase::Pending));
    }
    #[test]
    fn edit_revision_or_even_whitespace_invalidates_before_ready() {
        for (revision, source) in [("2", "s"), ("1", "s\n")] {
            let mut s = Leases::<()>::default(); let w = s.begin("main", "1", "s").unwrap();
            s.observe_snapshot("main", "1", "s");
            assert!(!w.cancellation().load(Ordering::Acquire));
            s.observe_snapshot("main", revision, source);
            assert!(w.cancellation().load(Ordering::Acquire));
            assert_eq!(s.ready("main", &w, ()), Err(Error::Stale));
        }
    }
    #[test]
    fn late_chooser_and_old_finally_cannot_replace_or_cancel_new_work() {
        let mut s = Leases::<()>::default(); let old = s.begin("main", "1", "s").unwrap();
        assert!(matches!(s.begin("main", "1", "s"), Err(Error::Busy)));
        s.close("main"); let new = s.begin("main", "2", "s2").unwrap();
        assert_eq!(s.ready("main", &old, ()), Err(Error::Stale));
        s.cancel("main", &old);
        assert_eq!(s.phase("main", &new), Ok(Phase::Pending));
    }
    #[test]
    fn edit_during_render_prevents_publication() {
        let mut s = Leases::default(); let w = s.begin("main", "1", "s").unwrap();
        s.ready("main", &w, ()).unwrap(); s.start_render("main", &w).unwrap();
        s.observe_snapshot("main", "2", "s2");
        assert_eq!(s.publish("main", &w, || panic!("must not publish")), Err(Error::Stale));
    }
    #[test]
    fn publish_failure_and_panic_retire_and_registry_drop_cancels() {
        let mut s = Leases::default(); let w = s.begin("main", "1", "s").unwrap();
        s.ready("main", &w, ()).unwrap(); s.start_render("main", &w).unwrap();
        assert_eq!(s.publish("main", &w, || Err::<(), _>("io")), Ok(Err("io")));
        let w = s.begin("main", "1", "s").unwrap();
        s.ready("main", &w, ()).unwrap(); s.start_render("main", &w).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = s.publish("main", &w, || panic!("writer failure"));
        })).is_err());
        assert!(w.cancellation().load(Ordering::Acquire));
        let w = s.begin("main", "1", "s").unwrap(); drop(s);
        assert!(w.cancellation().load(Ordering::Acquire));
    }
    #[test]
    fn cancel_wins_before_publication_lock_and_worker_observes_it() {
        use std::sync::{Mutex, mpsc};
        let mut leases = Leases::default();
        let work = leases.begin("main", "1", "s").unwrap();
        leases.ready("main", &work, ()).unwrap();
        leases.start_render("main", &work).unwrap();
        let state = Arc::new(Mutex::new(leases));
        let (go, wait) = mpsc::channel();
        let child_state = state.clone(); let child_work = work.clone();
        let worker = std::thread::spawn(move || {
            wait.recv().unwrap();
            assert!(child_work.cancellation().load(Ordering::Acquire));
            child_state.lock().unwrap().publish("main", &child_work, || panic!("cancelled publish"))
        });
        state.lock().unwrap().cancel("main", &work);
        go.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Err(Error::Stale));
    }
    #[test]
    fn accepted_publication_is_not_retroactively_cancelled() {
        // This tests lock ordering only, not the currently unconnected WAV writer.
        use std::sync::{Mutex, mpsc};
        let mut leases = Leases::default();
        let work = leases.begin("main", "1", "s").unwrap();
        leases.ready("main", &work, ()).unwrap();
        leases.start_render("main", &work).unwrap();
        let state = Arc::new(Mutex::new(leases));
        let (entered, wait) = mpsc::channel();
        let child_state = state.clone(); let child_work = work.clone();
        let cancel = std::thread::spawn(move || {
            wait.recv().unwrap();
            child_state.lock().unwrap().cancel("main", &child_work);
        });
        let result = state.lock().unwrap().publish("main", &work, || {
            entered.send(()).unwrap();
            // Cancellation cannot enter the registry during this commit closure.
            assert!(!work.cancellation().load(Ordering::Acquire));
            "published"
        });
        cancel.join().unwrap();
        assert_eq!(result, Ok("published"));
        assert_eq!(state.lock().unwrap().phase("main", &work), Err(Error::Stale));
    }
    #[test]
    fn generation_exhaustion_does_not_wrap_or_create_slot() {
        let mut s = Leases::<()>::default(); s.next = u64::MAX;
        assert!(matches!(s.begin("main", "1", "s"), Err(Error::Exhausted)));
        assert!(s.windows.is_empty());
    }
}
