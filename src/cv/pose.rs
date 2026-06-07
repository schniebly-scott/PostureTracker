use crate::constants::{
    CONFIDENCE_THRESHOLD, INF_HEIGHT, INF_WIDTH, KEEP_KEYPOINTS, KPT_START, SKELETON,
};
use ndarray::{Array4, ArrayView3};
use std::{error::Error, fmt::Debug};

use raqote::{DrawOptions, DrawTarget, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle};
pub type Keypoints = [Option<(f32, f32, f32)>; 17];

#[derive(Debug)]

pub struct PoseTask {
    inf_width: usize,
    inf_height: usize,
    confidence_threshold: f32,
}

impl PoseTask {
    pub fn new() -> Self {
        Self {
            inf_width: INF_WIDTH,
            inf_height: INF_HEIGHT,
            confidence_threshold: CONFIDENCE_THRESHOLD,
        }
    }

    fn decode_yolo_pose(
        &self,
        preds: ArrayView3<f32>,
        orig_w: u32,
        orig_h: u32,
    ) -> Result<Keypoints, Box<dyn Error>> {
        // shape: [1, 56, 8400]
        let preds = preds.index_axis(ndarray::Axis(0), 0);

        // transpose to [8400, 56]
        let preds = preds.permuted_axes([1, 0]);

        let mut best_conf = 0.0;
        let mut best_row = None;

        for row in preds.outer_iter() {
            let conf = row[4];

            if conf > best_conf {
                best_conf = conf;
                best_row = Some(row);
            }
        }

        let row = best_row.ok_or("No detections")?;

        let mut keypoints: Keypoints = [None; 17];

        let scale_x = orig_w as f32 / self.inf_width as f32;
        let scale_y = orig_h as f32 / self.inf_height as f32;
        let kpt_start = KPT_START; // after bbox + obj + class

        for &k in &KEEP_KEYPOINTS {
            let base = kpt_start + k * 3;

            let x = row[base] * scale_x as f32;
            let y = row[base + 1] * scale_y as f32;
            let conf = row[base + 2];

            keypoints[k] = Some((x, y, conf));
        }

        Ok(keypoints)
    }

    fn render_pose(&self, keypoints: &Keypoints, width: u32, height: u32) -> Vec<u8> {
        // ----- Draw -----
        let mut dt = DrawTarget::new(width as i32, height as i32);
        self.draw_skeleton(&mut dt, &keypoints);

        // ----- Extract RGBA back out -----
        let data = dt.get_data();
        let mut out = Vec::with_capacity((width * height * 4) as usize);

        for px in data {
            out.push((px >> 16) as u8); // R
            out.push((px >> 8) as u8); // G
            out.push(*px as u8); // B
            out.push((px >> 24) as u8); // A
        }
        out
    }

    pub fn posture_angle_deg(&self, keypoints: &Keypoints) -> Option<f32> {
        let (nose_x, nose_y, nose_conf) = keypoints[0]?;
        let (left_x, left_y, left_conf) = keypoints[5]?;
        let (right_x, right_y, right_conf) = keypoints[6]?;

        if nose_conf < self.confidence_threshold
            || left_conf < self.confidence_threshold
            || right_conf < self.confidence_threshold
        {
            return None;
        }

        let left = (left_x - nose_x, left_y - nose_y);
        let right = (right_x - nose_x, right_y - nose_y);

        let left_mag = (left.0 * left.0 + left.1 * left.1).sqrt();
        let right_mag = (right.0 * right.0 + right.1 * right.1).sqrt();

        if left_mag <= f32::EPSILON || right_mag <= f32::EPSILON {
            return None;
        }

        let cos_theta = ((left.0 * right.0) + (left.1 * right.1)) / (left_mag * right_mag);
        let clamped = cos_theta.clamp(-1.0, 1.0);

        Some(clamped.acos().to_degrees())
    }

    fn draw_skeleton(&self, dt: &mut DrawTarget, keypoints: &Keypoints) {
        for &(i, j) in SKELETON {
            if let (Some((x1, y1, c1)), Some((x2, y2, c2))) = (keypoints[i], keypoints[j]) {
                if c1 < self.confidence_threshold || c2 < self.confidence_threshold {
                    continue;
                }

                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);

                dt.stroke(
                    &pb.finish(),
                    &Source::Solid(SolidSource {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    }),
                    &StrokeStyle {
                        width: 2.0,
                        join: LineJoin::Round,
                        ..Default::default()
                    },
                    &DrawOptions::new(),
                );
            }
        }

        for &(x, y, c) in keypoints.iter().flatten() {
            if c < self.confidence_threshold {
                continue;
            }

            let mut pb = PathBuilder::new();
            pb.arc(x, y, 4.0, 0.0, std::f32::consts::TAU);

            dt.fill(
                &pb.finish(),
                &Source::Solid(SolidSource {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                }),
                &DrawOptions::new(),
            );
        }
    }

    pub fn preprocess_rgba(&self, rgba: &[u8], width: u32, height: u32) -> Array4<f32> {
        let w = self.inf_width;
        let h = self.inf_height;

        let mut input = Array4::<f32>::zeros((1, 3, h, w));
        let out = input.as_slice_mut().unwrap();

        let hw = h * w;
        let scale = 1.0 / 255.0;

        let x_ratio = width as f32 / w as f32;
        let y_ratio = height as f32 / h as f32;

        for y in 0..h {
            let src_y = (y as f32 * y_ratio) as usize;

            for x in 0..w {
                let src_x = (x as f32 * x_ratio) as usize;

                let src_i = ((src_y * width as usize + src_x) * 4) as usize;
                let dst_i = y * w + x;

                let r = rgba[src_i];
                let g = rgba[src_i + 1];
                let b = rgba[src_i + 2];

                out[dst_i] = r as f32 * scale;
                out[hw + dst_i] = g as f32 * scale;
                out[2 * hw + dst_i] = b as f32 * scale;
            }
        }

        input
    }

    pub fn postprocess(
        &self,
        outputs: &ort::session::SessionOutputs,
        output_name: &str,
        orig_w: u32,
        orig_h: u32,
    ) -> Result<Keypoints, Box<dyn Error>> {
        let tensor = outputs.get(output_name).ok_or("Missing output tensor")?;

        let array = tensor.try_extract_array::<f32>()?;

        let preds = array.view().into_dimensionality::<ndarray::Ix3>()?;
        let keypoints = self.decode_yolo_pose(preds, orig_w, orig_h)?;
        Ok(keypoints)
    }

    pub fn render(&self, result: &Keypoints, width: u32, height: u32) -> Vec<u8> {
        self.render_pose(result, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array3;

    /// Builds a Keypoints array with only nose (0), left shoulder (5) and right
    /// shoulder (6) set — the three points posture_angle_deg actually reads.
    fn keypoints(
        nose: Option<(f32, f32, f32)>,
        left: Option<(f32, f32, f32)>,
        right: Option<(f32, f32, f32)>,
    ) -> Keypoints {
        let mut kp: Keypoints = [None; 17];
        kp[0] = nose;
        kp[5] = left;
        kp[6] = right;
        kp
    }

    #[test]
    fn angle_is_90_degrees_for_symmetric_shoulders() {
        // Nose at origin, shoulders at (-1,1) and (1,1): vectors are perpendicular.
        let task = PoseTask::new();
        let kp = keypoints(
            Some((0.0, 0.0, 1.0)),
            Some((-1.0, 1.0, 1.0)),
            Some((1.0, 1.0, 1.0)),
        );
        let angle = task.posture_angle_deg(&kp).expect("angle should be Some");
        assert!((angle - 90.0).abs() < 1e-3, "expected ~90°, got {angle}");
    }

    #[test]
    fn angle_is_0_degrees_for_parallel_vectors() {
        let task = PoseTask::new();
        let kp = keypoints(
            Some((0.0, 0.0, 1.0)),
            Some((1.0, 1.0, 1.0)),
            Some((2.0, 2.0, 1.0)),
        );
        let angle = task.posture_angle_deg(&kp).expect("angle should be Some");
        assert!(angle.abs() < 1e-3, "expected ~0°, got {angle}");
    }

    #[test]
    fn angle_is_180_degrees_for_opposite_vectors() {
        let task = PoseTask::new();
        let kp = keypoints(
            Some((0.0, 0.0, 1.0)),
            Some((-1.0, 0.0, 1.0)),
            Some((1.0, 0.0, 1.0)),
        );
        let angle = task.posture_angle_deg(&kp).expect("angle should be Some");
        assert!((angle - 180.0).abs() < 1e-3, "expected ~180°, got {angle}");
    }

    #[test]
    fn angle_is_none_when_keypoint_missing() {
        let task = PoseTask::new();
        let kp = keypoints(Some((0.0, 0.0, 1.0)), None, Some((1.0, 1.0, 1.0)));
        assert!(task.posture_angle_deg(&kp).is_none());
    }

    #[test]
    fn angle_is_none_when_confidence_below_threshold() {
        let task = PoseTask::new();
        // Left shoulder confidence below CONFIDENCE_THRESHOLD (0.05).
        let kp = keypoints(
            Some((0.0, 0.0, 1.0)),
            Some((-1.0, 1.0, 0.01)),
            Some((1.0, 1.0, 1.0)),
        );
        assert!(task.posture_angle_deg(&kp).is_none());
    }

    #[test]
    fn angle_is_none_for_zero_magnitude_vector() {
        let task = PoseTask::new();
        // Left shoulder coincident with nose => zero-length vector.
        let kp = keypoints(
            Some((0.0, 0.0, 1.0)),
            Some((0.0, 0.0, 1.0)),
            Some((1.0, 1.0, 1.0)),
        );
        assert!(task.posture_angle_deg(&kp).is_none());
    }

    #[test]
    fn angle_never_nan_for_near_collinear_inputs() {
        // Floating point can push cos(theta) slightly past 1.0; the clamp must
        // prevent acos from returning NaN.
        let task = PoseTask::new();
        let kp = keypoints(
            Some((0.0, 0.0, 1.0)),
            Some((1.000001, 1.0, 1.0)),
            Some((1.0, 1.0, 1.0)),
        );
        let angle = task.posture_angle_deg(&kp).expect("angle should be Some");
        assert!(!angle.is_nan(), "angle should not be NaN");
    }

    #[test]
    fn preprocess_rgba_produces_normalized_nchw_tensor() {
        let task = PoseTask::new();
        // 2x2 solid red image (R=255, G=0, B=0, A=255).
        let rgba: Vec<u8> = [255u8, 0, 0, 255].repeat(4);
        let out = task.preprocess_rgba(&rgba, 2, 2);

        assert_eq!(out.shape(), &[1, 3, INF_HEIGHT, INF_WIDTH]);

        // R channel normalized to 1.0, G and B to 0.0 at an interior pixel.
        assert!((out[[0, 0, 100, 100]] - 1.0).abs() < 1e-6);
        assert_eq!(out[[0, 1, 100, 100]], 0.0);
        assert_eq!(out[[0, 2, 100, 100]], 0.0);

        // All values within the normalized range.
        for &v in out.iter() {
            assert!((0.0..=1.0).contains(&v), "value {v} outside [0,1]");
        }
    }

    #[test]
    fn decode_yolo_pose_extracts_and_scales_best_detection() {
        let task = PoseTask::new();
        // Output tensor shape [1, 56, 8400]: 4 bbox + 1 conf + 17*3 keypoints.
        let mut preds = Array3::<f32>::zeros((1, 56, 8400));

        // Detection 0: high confidence, will be selected.
        preds[[0, 4, 0]] = 0.9;
        // nose (k=0): base = KPT_START + 0*3 = 5
        preds[[0, 5, 0]] = 100.0; // x
        preds[[0, 6, 0]] = 50.0; // y
        preds[[0, 7, 0]] = 0.8; // conf
        // left shoulder (k=5): base = 5 + 15 = 20
        preds[[0, 20, 0]] = 10.0;
        preds[[0, 21, 0]] = 20.0;
        preds[[0, 22, 0]] = 0.7;

        // Detection 1: lower confidence, should be ignored.
        preds[[0, 4, 1]] = 0.3;
        preds[[0, 5, 1]] = 999.0;

        // orig 1280x640 => scale_x = 2.0, scale_y = 1.0
        let kp = task
            .decode_yolo_pose(preds.view(), 1280, 640)
            .expect("should decode");

        let (x, y, conf) = kp[0].expect("nose present");
        assert!((x - 200.0).abs() < 1e-3, "nose x scaled: {x}");
        assert!((y - 50.0).abs() < 1e-3, "nose y scaled: {y}");
        assert!((conf - 0.8).abs() < 1e-6);

        let (lx, ly, _) = kp[5].expect("left shoulder present");
        assert!((lx - 20.0).abs() < 1e-3, "left x scaled: {lx}");
        assert!((ly - 20.0).abs() < 1e-3, "left y scaled: {ly}");
    }

    #[test]
    fn decode_yolo_pose_errors_when_no_detections() {
        let task = PoseTask::new();
        // All confidences are 0 => no detection above best_conf=0.0.
        let preds = Array3::<f32>::zeros((1, 56, 8400));
        assert!(task.decode_yolo_pose(preds.view(), 640, 640).is_err());
    }

    #[test]
    fn render_returns_rgba_buffer_of_expected_size() {
        let task = PoseTask::new();
        let kp = keypoints(
            Some((2.0, 2.0, 1.0)),
            Some((1.0, 5.0, 1.0)),
            Some((8.0, 5.0, 1.0)),
        );
        let out = task.render(&kp, 10, 10);
        assert_eq!(out.len(), 10 * 10 * 4);
    }
}
