use crate::constants::{
    CONFIDENCE_THRESHOLD, INF_HEIGHT, INF_WIDTH, KEEP_KEYPOINTS, KPT_START, SKELETON,
};
use ndarray::{Array4, ArrayView3};
use std::{error::Error, fmt::Debug};

use raqote::{DrawOptions, DrawTarget, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle};
pub type Keypoints = [Option<(f32, f32, f32)>; 17];

pub struct PoseTask {
    inf_width: usize,
    inf_height: usize,
    confidence_threshold: f32,
    /// Reusable ARGB backing buffer for the raqote draw target, kept across
    /// frames so each render reuses this ~width*height allocation instead of
    /// allocating a fresh one. `DrawTarget` itself isn't `Send` (and `PoseTask`
    /// is shared across threads), so we hold the `Vec` and rebuild a target
    /// around it per frame via `from_vec`/`into_vec`.
    draw_buf: Vec<u32>,
}

// Eliding draw_buf: deriving Debug would dump the entire pixel buffer.
impl Debug for PoseTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoseTask")
            .field("inf_width", &self.inf_width)
            .field("inf_height", &self.inf_height)
            .field("confidence_threshold", &self.confidence_threshold)
            .finish_non_exhaustive()
    }
}

/// Computes letterbox parameters for fitting an `orig_w`×`orig_h` image into an
/// `inf_w`×`inf_h` square while preserving aspect ratio. Returns the scaled
/// content size, the symmetric padding offsets, and the scale factor such that
/// `inference_coord = original_coord * ratio + pad`.
fn letterbox(orig_w: u32, orig_h: u32, inf_w: u32, inf_h: u32) -> (u32, u32, f32, f32, f32) {
    let ratio = (inf_w as f32 / orig_w as f32).min(inf_h as f32 / orig_h as f32);
    let new_w = (orig_w as f32 * ratio).round();
    let new_h = (orig_h as f32 * ratio).round();
    let pad_x = (inf_w as f32 - new_w) / 2.0;
    let pad_y = (inf_h as f32 - new_h) / 2.0;
    (new_w as u32, new_h as u32, pad_x, pad_y, ratio)
}

impl PoseTask {
    pub fn new() -> Self {
        Self {
            inf_width: INF_WIDTH,
            inf_height: INF_HEIGHT,
            confidence_threshold: CONFIDENCE_THRESHOLD,
            draw_buf: Vec::new(),
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

        // Pick the highest-confidence anchor; a zero best conf means no detection.
        let row = preds
            .outer_iter()
            .max_by(|a, b| a[4].total_cmp(&b[4]))
            .filter(|row| row[4] > 0.0)
            .ok_or("No detections")?;

        let mut keypoints: Keypoints = [None; 17];

        // Reverse the letterbox preprocess: subtract padding, then undo the scale.
        let (_, _, pad_x, pad_y, ratio) =
            letterbox(orig_w, orig_h, self.inf_width as u32, self.inf_height as u32);
        let kpt_start = KPT_START; // after bbox + obj + class

        for &k in &KEEP_KEYPOINTS {
            let base = kpt_start + k * 3;

            let x = (row[base] - pad_x) / ratio;
            let y = (row[base + 1] - pad_y) / ratio;
            let conf = row[base + 2];

            keypoints[k] = Some((x, y, conf));
        }

        Ok(keypoints)
    }

    fn render_pose(&mut self, keypoints: &Keypoints, width: u32, height: u32) -> Vec<u8> {
        let confidence_threshold = self.confidence_threshold;

        // ----- Draw onto a target backed by the reused buffer -----
        // from_vec resizes the buffer to width*height; clear then overwrites any
        // stale pixels, so the buffer's prior contents don't leak between frames.
        let buf = std::mem::take(&mut self.draw_buf);
        let mut dt = DrawTarget::from_vec(width as i32, height as i32, buf);
        dt.clear(SolidSource {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });

        Self::draw_skeleton(&mut dt, keypoints, confidence_threshold);

        // ----- Extract RGBA back out (ARGB u32 -> RGBA bytes), chunked -----
        let mut out = vec![0u8; (width * height * 4) as usize];
        for (px, dst) in dt.get_data().iter().zip(out.chunks_exact_mut(4)) {
            dst[0] = (px >> 16) as u8; // R
            dst[1] = (px >> 8) as u8; // G
            dst[2] = *px as u8; // B
            dst[3] = (px >> 24) as u8; // A
        }

        // Reclaim the buffer for the next frame.
        self.draw_buf = dt.into_vec();
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

    fn draw_skeleton(dt: &mut DrawTarget, keypoints: &Keypoints, confidence_threshold: f32) {
        for &(i, j) in SKELETON {
            if let (Some((x1, y1, c1)), Some((x2, y2, c2))) = (keypoints[i], keypoints[j]) {
                if c1 < confidence_threshold || c2 < confidence_threshold {
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
            if c < confidence_threshold {
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

        // Letterbox: preserve aspect ratio and pad the remainder with YOLO's gray
        // (114) instead of stretching, which would degrade keypoint confidence.
        // decode_yolo_pose reverses this exact transform.
        let (new_w, new_h, pad_x, pad_y, ratio) = letterbox(width, height, w as u32, h as u32);
        let pad_x = pad_x as usize;
        let pad_y = pad_y as usize;

        out.fill(114.0 * scale);

        for y in 0..new_h as usize {
            let src_y = (((y as f32) / ratio) as usize).min(height as usize - 1);
            let dst_row = y + pad_y;

            for x in 0..new_w as usize {
                let src_x = (((x as f32) / ratio) as usize).min(width as usize - 1);
                let dst_col = x + pad_x;

                let src_i = (src_y * width as usize + src_x) * 4;
                let dst_i = dst_row * w + dst_col;

                out[dst_i] = rgba[src_i] as f32 * scale;
                out[hw + dst_i] = rgba[src_i + 1] as f32 * scale;
                out[2 * hw + dst_i] = rgba[src_i + 2] as f32 * scale;
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

    pub fn render(&mut self, result: &Keypoints, width: u32, height: u32) -> Vec<u8> {
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
    fn decode_yolo_pose_extracts_and_undoes_letterbox() {
        let task = PoseTask::new();
        // Output tensor shape [1, 56, 8400]: 4 bbox + 1 conf + 17*3 keypoints.
        let mut preds = Array3::<f32>::zeros((1, 56, 8400));

        // orig 1280x640 letterboxed into 640x640:
        //   ratio = min(640/1280, 640/640) = 0.5
        //   content = 640x320, pad_x = 0, pad_y = (640-320)/2 = 160
        // Inference coords reverse as: orig = (coord - pad) / ratio.

        // Detection 0: high confidence, will be selected.
        preds[[0, 4, 0]] = 0.9;
        // nose (k=0): base = KPT_START + 0*3 = 5; inference-space center (320, 320).
        preds[[0, 5, 0]] = 320.0; // x -> (320 - 0) / 0.5 = 640
        preds[[0, 6, 0]] = 320.0; // y -> (320 - 160) / 0.5 = 320
        preds[[0, 7, 0]] = 0.8; // conf
        // left shoulder (k=5): base = 5 + 15 = 20; inference-space (100, 200).
        preds[[0, 20, 0]] = 100.0; // x -> (100 - 0) / 0.5 = 200
        preds[[0, 21, 0]] = 200.0; // y -> (200 - 160) / 0.5 = 80
        preds[[0, 22, 0]] = 0.7;

        // Detection 1: lower confidence, should be ignored.
        preds[[0, 4, 1]] = 0.3;
        preds[[0, 5, 1]] = 999.0;

        let kp = task
            .decode_yolo_pose(preds.view(), 1280, 640)
            .expect("should decode");

        let (x, y, conf) = kp[0].expect("nose present");
        assert!((x - 640.0).abs() < 1e-3, "nose x unletterboxed: {x}");
        assert!((y - 320.0).abs() < 1e-3, "nose y unletterboxed: {y}");
        assert!((conf - 0.8).abs() < 1e-6);

        let (lx, ly, _) = kp[5].expect("left shoulder present");
        assert!((lx - 200.0).abs() < 1e-3, "left x unletterboxed: {lx}");
        assert!((ly - 80.0).abs() < 1e-3, "left y unletterboxed: {ly}");
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
        let mut task = PoseTask::new();
        let kp = keypoints(
            Some((2.0, 2.0, 1.0)),
            Some((1.0, 5.0, 1.0)),
            Some((8.0, 5.0, 1.0)),
        );
        let out = task.render(&kp, 10, 10);
        assert_eq!(out.len(), 10 * 10 * 4);
    }

    #[test]
    fn render_reuses_target_across_different_frame_sizes() {
        // Reusing the backing buffer must still produce correctly sized output
        // when the frame size changes (buffer resizes) and when it stays the same.
        let mut task = PoseTask::new();
        let kp = keypoints(Some((2.0, 2.0, 1.0)), None, None);

        assert_eq!(task.render(&kp, 10, 10).len(), 10 * 10 * 4);
        assert_eq!(task.render(&kp, 20, 8).len(), 20 * 8 * 4);
        assert_eq!(task.render(&kp, 10, 10).len(), 10 * 10 * 4);
    }

    #[test]
    fn letterbox_centers_content_and_preserves_aspect() {
        // 1280x640 into 640x640: half scale, padded top and bottom.
        let (new_w, new_h, pad_x, pad_y, ratio) = letterbox(1280, 640, 640, 640);
        assert_eq!((new_w, new_h), (640, 320));
        assert_eq!((pad_x, pad_y), (0.0, 160.0));
        assert!((ratio - 0.5).abs() < 1e-6);

        // A square source needs no padding and maps 1:1.
        let (sw, sh, spx, spy, sratio) = letterbox(640, 640, 640, 640);
        assert_eq!((sw, sh), (640, 640));
        assert_eq!((spx, spy), (0.0, 0.0));
        assert!((sratio - 1.0).abs() < 1e-6);
    }

    #[test]
    fn preprocess_rgba_letterboxes_non_square_input() {
        let task = PoseTask::new();
        // Solid red 1280x640 frame; letterboxed into 640x640 the content occupies
        // rows 160..480, leaving gray (114/255) padding above and below.
        let rgba: Vec<u8> = [255u8, 0, 0, 255].repeat(1280 * 640);
        let out = task.preprocess_rgba(&rgba, 1280, 640);

        let gray = 114.0 / 255.0;
        // Padding row (10 < 160): all channels gray.
        assert!((out[[0, 0, 10, 300]] - gray).abs() < 1e-6);
        assert!((out[[0, 1, 10, 300]] - gray).abs() < 1e-6);
        assert!((out[[0, 2, 10, 300]] - gray).abs() < 1e-6);

        // Content row (300 within 160..480): red.
        assert!((out[[0, 0, 300, 300]] - 1.0).abs() < 1e-6);
        assert_eq!(out[[0, 1, 300, 300]], 0.0);
        assert_eq!(out[[0, 2, 300, 300]], 0.0);
    }
}
