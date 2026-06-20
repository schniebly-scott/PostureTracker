use std::sync::Arc;

use iced::Subscription;
use iced::advanced::subscription as iced_subscription;
use iced::advanced::subscription::Hasher;
use iced::futures::stream;
use iced::widget::image;

use tokio::sync::broadcast;

use crate::app::InferenceUpdate;
use crate::camera::RgbaBuffer;
use crate::cv::Inference;
use crate::utils::ManagedService;
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
            while let Ok(frame) = rx.recv().await {
                yield frame_handle(frame);
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
            while let Ok(inference) = rx.recv().await {
                yield InferenceUpdate {
                    handle: frame_handle(inference.frame),
                    time_metrics: inference.time_metrics,
                    posture_angle_deg: inference.posture_angle_deg,
                };
            }
        };
        Box::pin(s)
    }
}
