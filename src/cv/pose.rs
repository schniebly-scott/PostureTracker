use std::{error::Error, fmt::Debug};
use ndarray::{Array4, ArrayView3};
use crate::constants::{KPT_START, SKELETON, INF_HEIGHT, INF_WIDTH, CONFIDENCE_THRESHOLD, KEEP_KEYPOINTS};

use raqote::{
    DrawOptions, DrawTarget, LineJoin, PathBuilder,
    SolidSource, Source, StrokeStyle,
};
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
            out.push((px >> 8) as u8);  // G
            out.push(*px as u8);        // B
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
            if let (Some((x1, y1, c1)), Some((x2, y2, c2))) =
                (keypoints[i], keypoints[j])
            {
                if c1 < self.confidence_threshold || c2 < self.confidence_threshold {
                    continue;
                }

                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);

                dt.stroke(
                    &pb.finish(),
                    &Source::Solid(SolidSource { r: 255, g: 0, b: 0, a: 255 }),
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
                &Source::Solid(SolidSource { r: 0, g: 255, b: 0, a: 255 }),
                &DrawOptions::new(),
            );
        }
    }

    pub fn preprocess_rgba(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Array4<f32> {

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
        let tensor = outputs
            .get(output_name)
            .ok_or("Missing output tensor")?;

        let array = tensor.try_extract_array::<f32>()?;

        let preds = array.view().into_dimensionality::<ndarray::Ix3>()?;
        let keypoints = self.decode_yolo_pose(preds, orig_w, orig_h)?;
        Ok(keypoints)
    }

    pub fn render(
        &self,
        result: &Keypoints,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        self.render_pose(result, width, height)
    }
}
