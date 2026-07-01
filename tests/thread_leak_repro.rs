//! End-to-end repro for the worker-thread leak on stop→start restarts (see
//! issue_writeups/worker_thread_leak_on_restart.md). Ignored by default: it
//! needs a real camera at /dev/video0 and takes ~15 s. Run manually with
//! `cargo test --test thread_leak_repro -- --ignored --nocapture`.
//! Against the pre-fix code this fails with 4 -> 12 threads after 8 restarts.

use std::sync::Arc;
use std::time::Duration;

use posturetracker::camera::CameraManager;
use posturetracker::config::CameraConfig;
use posturetracker::frame_channel::FrameChannel;
use posturetracker::utils::ManagedService;

fn thread_count() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("Threads:"))
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[test]
#[ignore]
fn camera_restart_does_not_leak_threads() {
    let shared = Arc::new(FrameChannel::new());
    let config = CameraConfig {
        device: Some("/dev/video0".to_string()),
        capture_width: 640,
        capture_height: 480,
    };
    let manager = CameraManager::new(config, shared);

    manager.start().expect("first start");
    // Let the first session open the device and begin grabbing frames.
    std::thread::sleep(Duration::from_secs(2));
    let baseline = thread_count();

    // The leak scenario: stop immediately followed by start, as the app does
    // on every camera/resolution change. Previously each cycle re-armed the
    // old worker (it never saw the stop) and leaked one thread.
    for i in 0..8 {
        manager.stop();
        manager.start().unwrap_or_else(|e| panic!("restart {i}: {e}"));
        std::thread::sleep(Duration::from_millis(300));
    }

    // Give straggler threads time to drain out of grab_frame and exit.
    std::thread::sleep(Duration::from_secs(5));
    let after = thread_count();

    manager.stop();
    std::thread::sleep(Duration::from_secs(4));
    let stopped = thread_count();

    println!("threads: baseline={baseline} after-restarts={after} after-stop={stopped}");
    assert!(
        after <= baseline + 1,
        "thread count climbed across restarts: {baseline} -> {after}"
    );
    assert!(
        stopped <= baseline,
        "worker threads left hanging after stop: {baseline} -> {stopped}"
    );
}
