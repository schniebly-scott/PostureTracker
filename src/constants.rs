// COCO skeleton
pub const SKELETON: &[(usize, usize)] = &[
    (0, 1),
    (0, 2),
    (1, 3),
    (2, 4),
    (0, 5),
    (0, 6),
    (5, 7),
    (7, 9),
    (6, 8),
    (8, 10),
    (5, 11),
    (6, 12),
    (11, 12),
    (11, 13),
    (13, 15),
    (12, 14),
    (14, 16),
];

// --------- Keypoint descriptions -----------------
// 0: nose
// 1: left_eye
// 2: right_eye
// 3: left_ear
// 4: right_ear
// 5: left_shoulder
// 6: right_shoulder
// 7: left_elbow
// 8: right_elbow
// 9: left_wrist
// 10: right_wrist
// 11: left_hip
// 12: right_hip
// 13: left_knee
// 14: right_knee
// 15: left_ankle
// 16: right_ankle

// pub const COCO_KEYPOINT_NAMES: [&str; 17] = [
//     "nose",
//     "left_eye",
//     "right_eye",
//     "left_ear",
//     "right_ear",
//     "left_shoulder",
//     "right_shoulder",
//     "left_elbow",
//     "right_elbow",
//     "left_wrist",
//     "right_wrist",
//     "left_hip",
//     "right_hip",
//     "left_knee",
//     "right_knee",
//     "left_ankle",
//     "right_ankle",
// ];

pub const KPT_START: usize = 5;

pub const INF_WIDTH: usize = 640;
pub const INF_HEIGHT: usize = 640;
pub const CONFIDENCE_THRESHOLD: f32 = 0.05;
pub const KEEP_KEYPOINTS: [usize; 3] = [0, 5, 6];
pub static MODEL_BYTES: &[u8] = include_bytes!("../yolov8n-pose.onnx");
