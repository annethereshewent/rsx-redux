use super::scheduler::{EventType, Scheduler};

const CYCLES_PER_SCANLINE: usize = 3413;
const NUM_SCANLINES: usize = 263;

pub struct GPU {
    pub frame_finished: bool
}

impl GPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::FrameFinished, ((CYCLES_PER_SCANLINE as f32 * NUM_SCANLINES as f32) * (7.0/11.0)) as usize);

        Self {
            frame_finished: false
        }
    }

    pub fn handle_frame_finished(&mut self, scheduler: &mut Scheduler) {
        scheduler.schedule(EventType::FrameFinished, ((CYCLES_PER_SCANLINE as f32 * NUM_SCANLINES as f32) * (7.0/11.0)) as usize);

        self.frame_finished = true;
    }
}