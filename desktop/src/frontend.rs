use std::process::exit;

use sdl2::{controller::GameController, event::Event, video::Window, EventPump};

pub struct Frontend {
    window: Window,
    event_pump: EventPump,
    _controller: Option<GameController>
}

impl Frontend {
    pub fn new() -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

        let game_controller_subsystem = sdl_context.game_controller().unwrap();

        let available = game_controller_subsystem
            .num_joysticks()
            .map_err(|e| format!("can't enumerate joysticks: {}", e)).unwrap();

        let controller = (0..available)
            .find_map(|id| {
            match game_controller_subsystem.open(id) {
                Ok(c) => {
                    Some(c)
                }
                Err(_) => {
                    None
                }
            }
        });

        let window = video_subsystem
            .window("RSX-redux", (320 * 2) as u32, (240 * 2) as u32)
            .position_centered()
            .build()
            .unwrap();

        Self {
            window,
            event_pump: sdl_context.event_pump().unwrap(),
            _controller: controller
        }
    }

    pub fn handle_events(&mut self) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    exit(0);
                }
                _ => ()
            }
        }
    }
}