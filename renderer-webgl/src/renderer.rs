use js_sys::wasm_bindgen::JsCast;
use rsx_redux::cpu::bus::gpu::{CPUTransferParams, DisplayDepth, FillVramParams, GPUCommand, Polygon, TexturePageColors, VRamTransferParams, VramToVramTransferParams, GPU};
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext, WebGlTexture};

pub struct Renderer {
    canvas: HtmlCanvasElement,
    gl: WebGl2RenderingContext,
    vram_read: WebGlTexture,
    vram_write: WebGlTexture,
}

impl Renderer {
    pub fn new(canvas_id: &str) -> Self {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        let gl = canvas
            .get_context("webgl2")
            .unwrap()
            .unwrap()
            .dyn_into::<WebGl2RenderingContext>()
            .unwrap();

        let vram_read = gl.create_texture().unwrap();
        let vram_write = gl.create_texture().unwrap();

        Self {
            canvas,
            gl,
            vram_read,
            vram_write,
        }
    }

    pub fn process(&self, gpu: &mut GPU) {
        if gpu.commands_ready {
            gpu.commands_ready = false;
            if !gpu.gpu_commands.is_empty() {
                self.process_commands(gpu);
            }
        }
    }

    fn process_commands(&self, gpu: &mut GPU) {
        let mut sample_dirty = true;

        if gpu.display_depth == DisplayDepth::Bit24 {
            self.vram_writeback(gpu);
        }

        for command in gpu.gpu_commands.drain(..) {
            match command {
                GPUCommand::CPUtoVram(params) => {
                    self.execute_cpu_to_vram(params);
                }
                GPUCommand::VRAMtoCPU(params) => {
                    let halfwords = self.handle_cpu_transfer(params);

                    for halfword in halfwords {
                        gpu.gpuread_fifo.push_back(halfword);
                    }
                }
                GPUCommand::VramToVram(params) => {
                    self.execute_vram_to_vram(params);
                }
                GPUCommand::FillVRAM(params) => {
                    self.execute_fill_vram(params);
                }
                GPUCommand::RenderPolygon(polygon) => {
                    let is_16bpp = polygon.textured
                        && polygon.texpage.map(|texpage| texpage.texture_page_colors)
                            == Some(TexturePageColors::Bit15);

                    if is_16bpp && sample_dirty {
                        sample_dirty = false;

                        self.update_texture_for_sampling();
                    }

                    self.render_polygon(&polygon);

                    if !is_16bpp {
                        sample_dirty = true;
                    }
                }
            }
        }
    }

    fn vram_writeback(&self, gpu: &GPU) {

    }

    fn execute_cpu_to_vram(&self, params: VRamTransferParams) {

    }

    fn handle_cpu_transfer(&self, params: CPUTransferParams) -> Vec<u16> {
        Vec::new()
    }

    fn render_polygon(&self, Polygon: &Polygon) {

    }

    fn update_texture_for_sampling(&self) {

    }

    fn execute_fill_vram(&self, params: FillVramParams) {

    }

    fn execute_vram_to_vram(&self, params: VramToVramTransferParams) {

    }
}