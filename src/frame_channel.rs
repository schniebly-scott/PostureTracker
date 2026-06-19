use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

use crate::camera::Frame;

/// A single-slot "latest frame" channel shared between the camera worker
/// (producer) and the CV worker (consumer).
///
/// This replaces the old `Arc<Mutex<Option<Frame>>>` plus the CV worker's
/// 5 ms busy-wait poll. The consumer blocks on a condvar until a frame is
/// published or the pipeline is stopped, so an idle pipeline costs zero
/// wakeups. Only the most recent frame is retained: publishing overwrites a
/// frame the consumer hasn't taken yet, preserving the camera's "only the
/// latest frame matters" semantics.
#[derive(Debug)]
pub struct FrameChannel {
    slot: Mutex<Option<Frame>>,
    available: Condvar,
}

impl FrameChannel {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            available: Condvar::new(),
        }
    }

    /// Producer: publish the latest frame, replacing any frame not yet taken.
    pub fn publish(&self, frame: Frame) {
        let mut slot = self.slot.lock().unwrap();
        *slot = Some(frame);
        self.available.notify_one();
    }

    /// Wake any blocked consumer so it re-checks its `running` flag and exits.
    /// Called on shutdown. Taking the lock around `notify_all` ensures the
    /// notification can't be lost against a consumer that is about to wait.
    pub fn wake(&self) {
        let _slot = self.slot.lock().unwrap();
        self.available.notify_all();
    }

    /// Consumer: block until a frame is available, then return it. Returns
    /// `None` once `running` is false (the pipeline was stopped), so the caller
    /// exits its loop instead of polling the atomic.
    pub fn wait(&self, running: &AtomicBool) -> Option<Frame> {
        let mut slot = self.slot.lock().unwrap();
        loop {
            if !running.load(Ordering::SeqCst) {
                return None;
            }
            if let Some(frame) = slot.take() {
                return Some(frame);
            }
            slot = self.available.wait(slot).unwrap();
        }
    }
}

impl Default for FrameChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::camera::RgbaBuffer;

    fn dummy_frame(tag: u8) -> Frame {
        (1, 1, Arc::new(RgbaBuffer::unpooled(vec![tag, tag, tag, tag])))
    }

    #[test]
    fn publish_then_wait_returns_frame() {
        let channel = FrameChannel::new();
        let running = AtomicBool::new(true);
        channel.publish(dummy_frame(7));
        let frame = channel.wait(&running).expect("frame should be available");
        assert_eq!(frame.2.data, vec![7, 7, 7, 7]);
    }

    #[test]
    fn publish_overwrites_unconsumed_frame() {
        let channel = FrameChannel::new();
        let running = AtomicBool::new(true);
        channel.publish(dummy_frame(1));
        channel.publish(dummy_frame(2));
        // Only the latest survives; there is no queue.
        let frame = channel.wait(&running).unwrap();
        assert_eq!(frame.2.data, vec![2, 2, 2, 2]);
    }

    #[test]
    fn wait_returns_none_when_stopped() {
        let channel = Arc::new(FrameChannel::new());
        let running = Arc::new(AtomicBool::new(true));

        let consumer = {
            let channel = channel.clone();
            let running = running.clone();
            thread::spawn(move || channel.wait(&running).is_none())
        };

        // Give the consumer a moment to park on the condvar, then stop it.
        thread::sleep(Duration::from_millis(50));
        running.store(false, Ordering::SeqCst);
        channel.wake();

        assert!(consumer.join().unwrap(), "stopped wait should return None");
    }
}
