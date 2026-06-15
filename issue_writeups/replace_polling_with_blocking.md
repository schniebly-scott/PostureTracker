# Busy-wait polling loops should use blocking/async primitives

**Severity: low-medium — wasted wakeups and added latency; conflicts with the project's
low-resource goal.**

## Where
1. `src/cv/cv_worker.rs` ~line 66: when the `SharedFrame` slot is empty the worker does
   `thread::sleep(Duration::from_millis(5))` and re-polls. That's up to 200 wakeups/sec
   for a pipeline that produces ~30 frames/sec (or far fewer in interval mode).
2. `src/app/tray.rs` ~lines 151–162 (`TraySubscription::stream`): polls
   `MenuEvent::receiver().try_recv()` in a loop with `sleep(100ms)` forever — 10
   wakeups/sec for events that occur a few times per day. The poll also continues while
   the app is idle/minimized, which matters for the "background process" goal.

## Fixes
1. **CV worker:** replace `SharedFrame = Arc<Mutex<Option<Frame>>>` with
   `tokio::sync::watch::Sender<Option<Frame>>/Receiver` (camera `send`s, CV worker
   blocks on `changed()` via `blocking_recv`-style or keeps a small condvar), or a
   `std::sync::Condvar` paired with the existing Mutex. A `watch` channel preserves the
   "only the latest frame matters" semantics exactly (it overwrites, never queues) and
   removes both the sleep and the manual `take()`. Touch points: `src/lib.rs`
   (`SharedFrame` type + `new_app_state`), `src/camera/cam_worker.rs` (publish),
   `src/cv/cv_worker.rs` (consume). The worker shutdown should then select on
   `running`/channel-closed rather than polling the atomic — closing the watch sender on
   camera stop gives a natural exit signal.
2. **Tray:** `tray-icon`'s `MenuEvent` supports an event handler callback
   (`MenuEvent::set_event_handler`) — install one that forwards into a
   `tokio::sync::mpsc::UnboundedSender`, and make the subscription stream
   `rx.recv().await`. Zero polling, immediate response. Set the handler once in
   `TrayState::new`.

## Clues
- The subscription recipes hash only `TypeId` (`subscriptions.rs`, `tray.rs`), so iced
  keeps one stream instance alive for the app's lifetime — handing each recipe a
  receiver created at subscription time is safe today; if you ever make subscriptions
  restartable, the hash must incorporate something that changes (generation counter).
- Don't convert the *camera grab* loop to async — `ccap`'s `grab_frame(3000)` is a
  blocking C call and belongs on its dedicated thread. Only the idle-wait patterns are
  at issue.
