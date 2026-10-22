use std::{
    future::Future,
    io,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::sync::Notify;

pub struct AudioGuard {
    playing: AtomicBool,
    healthy: AtomicBool,
    changed: Notify,
}
impl AudioGuard {
    pub fn new() -> Self {
        Self {
            playing: AtomicBool::new(false),
            healthy: AtomicBool::new(false),
            changed: Notify::new(),
        }
    }
    pub fn set_playing(&self, playing: bool) {
        if self.playing.swap(playing, Ordering::AcqRel) != playing {
            self.healthy.store(false, Ordering::Release);
            self.changed.notify_one();
        }
    }
    pub fn healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }
    pub async fn run(&self) {
        self.run_with(crate::voice::pause_native).await;
    }
    async fn run_with<F, Fut>(&self, mut pause: F)
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = io::Result<()>>,
    {
        let mut tick = tokio::time::interval(Duration::from_millis(500));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! { _=tick.tick()=>{}, _=self.changed.notified()=>{} }
            if !self.playing.load(Ordering::Acquire) {
                continue;
            }
            let ok = pause().await.is_ok();
            if !ok && self.healthy.swap(false, Ordering::AcqRel) {
                eprintln!("audio: native music pause failed; retrying");
            } else if ok {
                self.healthy.store(true, Ordering::Release);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicUsize};
    #[tokio::test(start_paused = true)]
    async fn reclaims_late_native_playback_and_releases_when_paused() {
        let guard = Arc::new(AudioGuard::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let worker = {
            let g = guard.clone();
            let c = calls.clone();
            tokio::spawn(async move {
                g.run_with(|| {
                    c.fetch_add(1, Ordering::SeqCst);
                    async { Ok(()) }
                })
                .await;
            })
        };
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        guard.set_playing(true); // Connect restored after reboot, no voice event.
        tokio::task::yield_now().await;
        assert!(guard.healthy());
        let first = calls.load(Ordering::SeqCst);
        for _ in 0..6 {
            tokio::time::advance(Duration::from_millis(500)).await;
            tokio::task::yield_now().await;
        }
        assert!(calls.load(Ordering::SeqCst) >= first + 6);
        guard.set_playing(false);
        tokio::task::yield_now().await;
        let paused = calls.load(Ordering::SeqCst);
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), paused);
        guard.set_playing(true);
        tokio::task::yield_now().await;
        assert!(calls.load(Ordering::SeqCst) > paused);
        worker.abort();
    }
}
