use std::sync::Arc;

use iced::Subscription;
use iced::advanced::subscription as iced_subscription;
use iced::advanced::subscription::Hasher;
use iced::futures::stream;
use iced::widget::image;

use tokio::sync::{broadcast, watch};

use crate::app::InferenceUpdate;
use crate::camera::RgbaBuffer;
use crate::cv::Inference;
use crate::utils::{ManagedService, PipelineErrors};
use crate::{camera::CameraManager, cv::CVManager};
use crate::Frame;

/// Wraps a pooled `RgbaBuffer` so its pixels can back a zero-copy
/// `bytes::Bytes` (and thus an `image::Handle`) without cloning. The pooled
/// buffer returns to the pool only once iced drops the resulting `Handle`.
struct FrameBytes(Arc<RgbaBuffer>);

impl AsRef<[u8]> for FrameBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0.data
    }
}

/// Build an `image::Handle` that borrows the pooled frame buffer instead of
/// copying its pixels.
fn frame_handle(frame: Frame) -> image::Handle {
    image::Handle::from_rgba(frame.0, frame.1, bytes::Bytes::from_owner(FrameBytes(frame.2)))
}

/* ============================
Camera Subscription
============================ */

pub fn raw_frame_subscription(camera_manager: Arc<CameraManager>) -> Subscription<image::Handle> {
    let service_id = Arc::as_ptr(&camera_manager) as usize;
    let rx = camera_manager.subscribe();
    iced_subscription::from_recipe(CameraSubscription::new(rx, service_id))
}

struct CameraSubscription {
    rx: broadcast::Receiver<Frame>,
    service_id: usize,
}

impl CameraSubscription {
    pub fn new(rx: broadcast::Receiver<Frame>, service_id: usize) -> Self {
        Self { rx, service_id }
    }
}

impl iced_subscription::Recipe for CameraSubscription {
    type Output = image::Handle;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
        // Keep subscriptions from distinct manager instances from deduplicating.
        self.service_id.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        let mut rx = self.rx;

        let s = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(frame) => yield frame_handle(frame),
                    // A live feed only wants the newest frame. If the UI briefly
                    // falls behind and the bounded broadcast drops intermediate
                    // frames, skip to the latest and keep going rather than
                    // ending the stream — a terminated stream tears down the
                    // subscription and stalls the feed (visible as a flickering,
                    // non-constant feed). This lag is far more likely at higher
                    // capture resolutions, where each frame is larger and the UI
                    // drains the channel more slowly.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };
        Box::pin(s)
    }
}

/* ============================
Pipeline Error Subscription
============================ */

/// Streams worker-thread failure reports into the update loop so pipeline
/// errors surface in the UI instead of dying on stderr.
pub fn pipeline_error_subscription(errors: &PipelineErrors) -> Subscription<String> {
    iced_subscription::from_recipe(PipelineErrorSubscription {
        rx: errors.subscribe(),
        service_id: errors.id(),
    })
}

struct PipelineErrorSubscription {
    rx: watch::Receiver<Option<String>>,
    service_id: usize,
}

impl iced_subscription::Recipe for PipelineErrorSubscription {
    type Output = String;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
        // Keep subscriptions from distinct error channels from deduplicating.
        self.service_id.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        let mut rx = self.rx;

        let s = async_stream::stream! {
            // `changed` skips the channel's initial empty slot, so only real
            // reports are yielded; a closed channel ends the stream.
            while rx.changed().await.is_ok() {
                let message = rx.borrow_and_update().clone();
                if let Some(message) = message {
                    yield message;
                }
            }
        };
        Box::pin(s)
    }
}

/* ============================
CV Subscription
============================ */

pub fn inference_subscription(
    cv_manager: Arc<CVManager>,
) -> Subscription<InferenceUpdate> {
    let service_id = Arc::as_ptr(&cv_manager) as usize;
    let rx = cv_manager.subscribe();
    iced_subscription::from_recipe(CVSubscription::new(rx, service_id))
}

struct CVSubscription {
    rx: broadcast::Receiver<Inference>,
    service_id: usize,
}

impl CVSubscription {
    pub fn new(rx: broadcast::Receiver<Inference>, service_id: usize) -> Self {
        Self { rx, service_id }
    }
}

impl iced_subscription::Recipe for CVSubscription {
    type Output = InferenceUpdate;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
        // Keep subscriptions from distinct manager instances from deduplicating.
        self.service_id.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        let mut rx = self.rx;

        let s = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(inference) => yield InferenceUpdate {
                        handle: frame_handle(inference.frame),
                        time_metrics: inference.time_metrics,
                        posture_angle_deg: inference.posture_angle_deg,
                    },
                    // Skip dropped results on lag rather than ending the stream
                    // (which would stop inference updates entirely); the next
                    // frame's result is what we want anyway.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };
        Box::pin(s)
    }
}
