use std::{sync::Arc};

use iced::Subscription;
use iced::advanced::subscription as iced_subscription;
use iced::advanced::subscription::Hasher;
use iced::futures::stream;
use iced::widget::image;

use tokio::sync::broadcast;

use crate::Frame;
use crate::camera::RgbaBuffer;
use crate::cv::{Inference, TimeMetrics};
use crate::utils::ManagedService;
use crate::{camera::CameraManager, cv::CVManager};

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
    let rx = camera_manager.subscribe();
    iced_subscription::from_recipe(CameraSubscription::new(rx))
}

struct CameraSubscription {
    rx: broadcast::Receiver<Frame>,
}

impl CameraSubscription {
    pub fn new(rx: broadcast::Receiver<Frame>) -> Self {
        Self { rx }
    }
}

impl iced_subscription::Recipe for CameraSubscription {
    type Output = image::Handle;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
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

/// Fatal camera errors that occur after startup (for example, unplugging the
/// device) are delivered separately from frames so the UI can stop the session
/// and explain what happened.
pub fn camera_failure_subscription(camera_manager: Arc<CameraManager>) -> Subscription<String> {
    let rx = camera_manager.subscribe_failures();
    iced_subscription::from_recipe(CameraFailureSubscription { rx })
}

struct CameraFailureSubscription {
    rx: broadcast::Receiver<String>,
}

impl iced_subscription::Recipe for CameraFailureSubscription {
    type Output = String;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        let mut rx = self.rx;

        let s = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(error) => yield error,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
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
) -> Subscription<(image::Handle, TimeMetrics, Option<f32>)> {
    let rx = cv_manager.subscribe();
    iced_subscription::from_recipe(CVSubscription::new(rx))
}

struct CVSubscription {
    rx: broadcast::Receiver<Inference>,
}

impl CVSubscription {
    pub fn new(rx: broadcast::Receiver<Inference>) -> Self {
        Self { rx }
    }
}

impl iced_subscription::Recipe for CVSubscription {
    type Output = (image::Handle, TimeMetrics, Option<f32>);
    //TODO: make the output inferred from Inference type but still transform frame to handle

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        let mut rx = self.rx;

        let s = async_stream::stream! {
            while let Ok(inference) = rx.recv().await {
                yield (
                    frame_handle(inference.frame),
                    inference.time_metrics,
                    inference.posture_angle_deg,
                );
            }
        };
        Box::pin(s)
    }
}
