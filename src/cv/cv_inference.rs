use std::{error::Error, time::Instant};

use ort::{
    inputs,
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};

use crate::cv::{TimeMetrics, pose::PoseTask};

#[derive(Debug)]
pub struct Model {
    session: Session,
    task: PoseTask,
    input_name: String,
    output_name: String,
}

impl Model {
    pub fn new() -> ort::Result<Self> {
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_memory(crate::constants::MODEL_BYTES)?;

        let task = PoseTask::default();

        let input_name = session.inputs()[0].name().to_string();
        let output_name = session.outputs()[0].name().to_string();

        Ok(Self {
            session,
            task,
            input_name,
            output_name,
        })
    }

    pub fn process_rgba(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(Vec<u8>, TimeMetrics, Option<f32>), Box<dyn Error>> {
        let t0 = Instant::now();
        let input = self.task.preprocess_rgba(rgba, width, height);
        let preprocess = t0.elapsed();

        let t1 = Instant::now();
        let outputs = self
            .session
            .run(inputs![&self.input_name => TensorRef::from_array_view(&input)?])?;
        let inference = t1.elapsed();

        let t2 = Instant::now();
        let result = self
            .task
            .postprocess(&outputs, &self.output_name, width, height)?;
        let postprocess = t2.elapsed();

        let t3 = Instant::now();
        let img = self.task.render(&result, width, height);
        let render = t3.elapsed();

        let posture_angle_deg = self.task.posture_angle_deg(&result);
        Ok((img, TimeMetrics {
            preprocess,
            postprocess,
            inference,
            render
        },
        posture_angle_deg))
    }
}
