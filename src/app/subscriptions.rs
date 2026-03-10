use std::{sync::Arc, time::Duration};

use iced::advanced::subscription::Hasher;
use iced::futures::stream;
use iced::advanced::subscription as iced_subscription;
use iced::widget::image;
use iced::Subscription;

use tokio::sync::broadcast;

use crate::{camera::CameraManager, cv::CVManager};
use crate::utils::ManagedService;

use super::{Frame, Inference};

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
                yield image::Handle::from_rgba(frame.0, frame.1, frame.2.data.clone());
            }
        };
        Box::pin(s)
    }
}

/* ============================
   CV Subscription
   ============================ */

pub fn inference_subscription(cv_manager: Arc<CVManager>) -> Subscription<(image::Handle, Duration)> {
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
    type Output = (image::Handle, Duration);
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
                let frame = inference.frame;
                yield (image::Handle::from_rgba(frame.0, frame.1, frame.2.data.clone()), inference.inf_time);
            }
        };
        Box::pin(s)
    }
}