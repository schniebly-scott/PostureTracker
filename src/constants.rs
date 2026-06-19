// Drawable skeleton edges. Only the keypoints in KEEP_KEYPOINTS (nose + both
// shoulders) are decoded, so the overlay can only ever connect those points;
// edges referencing undecoded keypoints would never render. This is the COCO
// subset that is actually drawable: nose->left_shoulder and nose->right_shoulder.
pub const SKELETON: &[(usize, usize)] = &[(0, 5), (0, 6)];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypoint_constants_match_coco_layout() {
        // nose, left_shoulder, right_shoulder — the three points posture math relies on.
        assert_eq!(KEEP_KEYPOINTS, [0, 5, 6]);
        assert_eq!(KPT_START, 5);
    }

    #[test]
    fn inference_dimensions_are_square_640() {
        assert_eq!(INF_WIDTH, 640);
        assert_eq!(INF_HEIGHT, 640);
    }

    #[test]
    fn confidence_threshold_is_expected() {
        assert_eq!(CONFIDENCE_THRESHOLD, 0.05);
    }

    #[test]
    fn skeleton_indices_are_within_keypoint_array() {
        // Every edge must reference a valid index into the 17-element Keypoints array,
        // otherwise draw_skeleton would panic at runtime.
        for &(i, j) in SKELETON {
            assert!(i < 17, "skeleton edge start {i} out of range");
            assert!(j < 17, "skeleton edge end {j} out of range");
        }
    }

    #[test]
    fn model_bytes_are_embedded() {
        assert!(!MODEL_BYTES.is_empty(), "ONNX model should be embedded");
    }
}
