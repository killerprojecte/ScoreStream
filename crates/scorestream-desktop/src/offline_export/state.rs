use super::lease::{Leases, Work};
const OWNER: &str = "main";
#[derive(Default)]
pub struct Host {
    pub(super) leases: Leases<()>,
    active: Option<(String, Work)>,
    next: u64,
    pub(super) running: bool,
}
impl Host {
    pub(super) fn begin(&mut self, revision: &str, source: &str) -> Result<String, String> {
        self.next = self.next.checked_add(1).ok_or("OFFLINE_GENERATION")?;
        let work = self.leases.begin(OWNER, revision, source).map_err(|_| "OFFLINE_BUSY")?;
        let id = format!("offline-{:016x}", self.next);
        self.active = Some((id.clone(), work));
        self.running = false;
        Ok(id)
    }
    pub(super) fn work(&self, id: &str) -> Result<Work, String> {
        self.active.as_ref().filter(|(current, _)| current == id)
            .map(|(_, work)| work.clone()).ok_or_else(|| "OFFLINE_STALE".into())
    }
    pub(super) fn cancel(&mut self, id: &str) {
        if let Ok(work) = self.work(id) { self.leases.cancel(OWNER, &work); self.active = None; self.running = false; }
    }
    pub(super) fn close(&mut self) { self.leases.close(OWNER); self.active = None; self.running = false; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    #[test]
    fn host_ids_do_not_allow_old_finally_to_cancel_new_reservation() {
        let mut h = Host::default(); let old = h.begin("r1", "s").unwrap();
        let work = h.work(&old).unwrap(); h.close();
        assert!(work.cancellation().load(Ordering::Acquire));
        let new = h.begin("r2", "s").unwrap(); h.cancel(&old);
        assert!(h.work(&new).is_ok()); assert!(h.work(&old).is_err());
        assert!(h.begin("r2", "s").is_err());
    }
    #[test]
    fn same_text_new_revision_and_reset_invalidate_host_work() {
        let mut h = Host::default(); let id = h.begin("r1", "s").unwrap();
        let work = h.work(&id).unwrap();
        h.leases.observe_snapshot(OWNER, "r2", "s");
        assert!(h.leases.phase(OWNER, &work).is_err());
        assert!(work.cancellation().load(Ordering::Acquire));
        h.cancel(&id); assert!(h.begin("r1", "s").is_ok());
    }
}
