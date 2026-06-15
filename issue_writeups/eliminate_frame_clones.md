# Full-frame `Vec<u8>` clones on every frame in the UI and CV paths

**Severity: medium-high — three avoidable ~1.2 MB copies per frame, two of them at camera
frame rate (30 fps ≈ 70 MB/s of memcpy + allocation).**

## Where
1. `src/app/subscriptions.rs` ~line 51 (`CameraSubscription::stream`):
   `image::Handle::from_rgba(frame.0, frame.1, frame.2.data.clone())`
2. `src/app/subscriptions.rs` ~line 98 (`CVSubscription::stream`): same pattern for the
   inference overlay.
3. `src/cv/cv_worker.rs` ~line 41:
   `let (width, height, rgba) = (frame.0, frame.1, frame.2.data.clone());` — clones the
   whole camera frame only to pass `&rgba` to `Model::process_rgba(&self, rgba: &[u8], ..)`.

## Problem
The whole point of `RgbaBuffer` + the buffer pool (`src/camera/cam_service.rs`) is to
avoid per-frame allocation, but every consumer immediately deep-copies the pixels back
out of the pooled buffer:

- The two subscription bridges clone the full RGBA buffer into a fresh `Vec` to build an
  `image::Handle`, every frame, at capture rate.
- The CV worker clones the input frame even though inference only needs a borrow.

## Fixes
**(3) is trivial:** delete the clone and borrow directly:
```rust
let (width, height) = (frame.0, frame.1);
match model.process_rgba(&frame.2.data, width, height) { ... }
```
No lifetime issues — `frame` lives across the call.

**(1)/(2) — zero-copy `image::Handle`:** iced 0.14's `Handle::from_rgba` accepts
`impl Into<bytes::Bytes>`. `bytes::Bytes::from_owner` (bytes ≥ 1.9) can wrap the existing
`Arc<RgbaBuffer>` without copying. Needs a small shim because `from_owner` requires
`AsRef<[u8]> + Send + 'static`:

```rust
// in src/camera/cam_service.rs or subscriptions.rs
struct FrameBytes(Arc<RgbaBuffer>);
impl AsRef<[u8]> for FrameBytes {
    fn as_ref(&self) -> &[u8] { &self.0.data }
}
// usage:
image::Handle::from_rgba(frame.0, frame.1, bytes::Bytes::from_owner(FrameBytes(frame.2)))
```
Add `bytes = "1"` to `Cargo.toml` (it's already in the tree as a transitive dep of tokio).

## Gotchas / clues
- **Pool interaction:** with zero-copy handles, the pooled buffer returns to the pool
  only when iced drops the `Handle` (it keeps the last one per `image` widget plus its
  internal cache generation). The camera pool may therefore hold a couple more buffers in
  flight than today — that's fine, the pool allocates on miss. Do NOT mutate a pooled
  buffer that may still be referenced: the current design never mutates after publish
  (`cam_worker` pops a *different* buffer for the next frame), so this is safe as-is, but
  preserve that invariant.
- The CV-side handle fix interacts with `cv_buffer_pool_leak.md` — if the CV pool is
  removed there, the `FrameBytes` shim still works unchanged.
- After the change, `Frame`'s `Arc<RgbaBuffer>` is shared by SharedFrame slot, broadcast
  channel, and UI handle simultaneously. `RgbaBuffer::drop` (pool return) fires once the
  last clone drops — exactly the desired behavior.
