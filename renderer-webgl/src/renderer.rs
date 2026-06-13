use std::cmp;

use bytemuck::{cast_slice, Pod, Zeroable};
use js_sys::{wasm_bindgen::JsCast, Float32Array};
use rsx_redux::cpu::bus::gpu::{CPUTransferParams, DisplayDepth, FillVramParams, GPUCommand, Polygon, TexturePageColors, VRamTransferParams, VramToVramTransferParams, GPU, SCREEN_HEIGHT, SCREEN_WIDTH, VRAM_HEIGHT, VRAM_WIDTH};
use web_sys::{window, Document, HtmlCanvasElement, WebGl2RenderingContext, WebGlBuffer, WebGlFramebuffer, WebGlProgram, WebGlShader, WebGlTexture};


const BYTE_LEN: usize = 4 * std::mem::size_of::<FbVertex>();
const QUAD_VERTS: [f32; 24] = [
    // pos        uv
    -1.0, -1.0,   0.0, 0.0,
     1.0, -1.0,   1.0, 0.0,
    -1.0,  1.0,   0.0, 1.0,
    -1.0,  1.0,   0.0, 1.0,
     1.0, -1.0,   1.0, 0.0,
     1.0,  1.0,   1.0, 1.0,
];

#[derive(PartialEq)]
enum TextureType {
    Read,
    Write,
    Blend,
}

#[repr(C)]
struct FbParams {
    display_start_x: u32,
    display_start_y: u32,
    display_width: u32,
    display_height: u32,
    display_depth: u32,
}

#[repr(C)]
#[derive(Debug)]
struct FragmentUniform {
    has_texture: bool,
    semitransparent: bool,
    modulate: bool,
    texture_mask_x: u32,
    texture_mask_y: u32,
    texture_offset_x: u32,
    texture_offset_y: u32,
    depth: i32,
    transparent_mode: u32,
    pass: u32,
    page: [u32; 2],
    clut: [u32; 2],
    force_mask_bit: bool,
    preserve_masked_pixels: bool,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GlVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    orig: [f32; 2],
}

impl GlVertex {
    pub fn new() -> Self {
        Self {
            position: [0.0; 2],
            uv: [0.0; 2],
            color: [0.0; 4],
            orig: [0.0; 2],
        }
    }
}

#[derive(Debug)]
struct Region {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct FbVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

pub struct Renderer {
    canvas: HtmlCanvasElement,
    gl: WebGl2RenderingContext,
    vram_read: WebGlTexture,
    vram_write: WebGlTexture,
    program: WebGlProgram,
    vertex_buffer: WebGlBuffer,
    quad_buffer: WebGlBuffer,
    fbo_write: WebGlFramebuffer,
    fb_program: WebGlProgram,
}

impl Renderer {
    pub fn new(canvas_id: &str) -> Self {
        let document = window().unwrap().document().unwrap();
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

        let fragment_shader_str = include_str!("shaders/fragment.glsl");
        let vertex_shader_str = include_str!("shaders/vertex.glsl");
        let fb_frag_shader_str = include_str!("shaders/fragment_fb.glsl");
        let fb_vert_shader_str = include_str!("shaders/vertex_fb.glsl");

        let fragment_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            fragment_shader_str
        ).unwrap();
        let vertex_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::VERTEX_SHADER,
            vertex_shader_str
        ).unwrap();
        let fb_frag_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            fb_frag_shader_str
        ).unwrap();
        let fb_vert_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::VERTEX_SHADER,
            fb_vert_shader_str
        ).unwrap();

        let program = Self::link_program(&gl, &vertex_shader, &fragment_shader).unwrap();
        let fb_program = Self::link_program(&gl, &fb_vert_shader, &fb_frag_shader).unwrap();

        let vertex_buffer = gl.create_buffer().unwrap();
        let quad_buffer = gl.create_buffer().unwrap();

        gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&vram_write));

        gl.tex_storage_2d(
            WebGl2RenderingContext::TEXTURE_2D,
            1,
            WebGl2RenderingContext::RGBA8,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
        );

        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_MIN_FILTER, WebGl2RenderingContext::NEAREST as i32);
        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_MAG_FILTER, WebGl2RenderingContext::NEAREST as i32);
        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_WRAP_S, WebGl2RenderingContext::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_WRAP_T, WebGl2RenderingContext::CLAMP_TO_EDGE as i32);

        gl.active_texture(WebGl2RenderingContext::TEXTURE1);
        gl.bind_texture(WebGl2RenderingContext::TEXTURE1, Some(&vram_read));

        gl.tex_storage_2d(
            WebGl2RenderingContext::TEXTURE_2D,
            1,
            WebGl2RenderingContext::R16UI,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
        );

        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_MIN_FILTER, WebGl2RenderingContext::NEAREST as i32);
        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_MAG_FILTER, WebGl2RenderingContext::NEAREST as i32);
        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_WRAP_S, WebGl2RenderingContext::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(WebGl2RenderingContext::TEXTURE_2D, WebGl2RenderingContext::TEXTURE_WRAP_T, WebGl2RenderingContext::CLAMP_TO_EDGE as i32);

        let fbo_write = gl.create_framebuffer().unwrap();

        gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&fbo_write));
        gl.framebuffer_texture_2d(
            WebGl2RenderingContext::FRAMEBUFFER,
            WebGl2RenderingContext::COLOR_ATTACHMENT0,
            WebGl2RenderingContext::TEXTURE_2D,
            Some(&vram_write),
            0,
        );

        Self {
            canvas,
            gl,
            vram_read,
            vram_write,
            program,
            vertex_buffer,
            quad_buffer,
            fbo_write,
            fb_program,
        }
    }

    fn compile_shader(
        gl: &WebGl2RenderingContext,
        shader_type: u32,
        source: &str
    ) -> Result<WebGlShader, String> {
        let shader = gl
            .create_shader(shader_type)
            .ok_or("Unable to create shader object".to_string())?;

        gl.shader_source(&shader, source);
        gl.compile_shader(&shader);

        if gl
            .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            Ok(shader)
        } else {
            Err(gl
                .get_shader_info_log(&shader)
                .unwrap_or("unknown error creating shader".to_string())
            )
        }
    }

    fn link_program(
        gl: &WebGl2RenderingContext,
        vertex_shader: &WebGlShader,
        fragment_shader: &WebGlShader,
    ) -> Result<WebGlProgram, String> {
        let program = gl
            .create_program()
            .ok_or("Unable to create gl program")?;

        gl.attach_shader(&program, vertex_shader);
        gl.attach_shader(&program, fragment_shader);
        gl.link_program(&program);

        if gl
            .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            Ok(program)
        } else {
            Err(gl
                .get_program_info_log(&program)
                .unwrap_or("Unable to create gl program for unknown reason".to_string())
            )
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

    pub fn clear_color(&self) {
        self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
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

    fn render_polygon(&self, polygon: &Polygon) {
        let mut vertices: Vec<GlVertex> = vec![GlVertex::new(); polygon.vertices.len()];

        let depth = if let Some(texpage) = polygon.texpage {
            match texpage.texture_page_colors {
                TexturePageColors::Bit4 => 0,
                TexturePageColors::Bit8 => 1,
                TexturePageColors::Bit15 => 2,
            }
        } else {
            -1
        };

        let mut fragment_uniform = FragmentUniform {
            has_texture: polygon.textured,
            texture_mask_x: polygon.texture_mask_x,
            texture_mask_y: polygon.texture_mask_y,
            texture_offset_x: polygon.texture_offset_x,
            texture_offset_y: polygon.texture_offset_y,
            semitransparent: polygon.semitransparent,
            modulate: polygon.modulate,
            depth,
            transparent_mode: polygon.transparent_mode,
            pass: 1,
            page: [0; 2],
            clut: [polygon.clut.0, polygon.clut.1],
            preserve_masked_pixels: polygon.preserve_masked_pixels,
            force_mask_bit: polygon.force_mask_bit,
        };

        let v = &polygon.vertices;

        if !polygon.is_line {
            let cross_product = GPU::cross_product(&polygon.vertices);
            if cross_product == 0 {
                return;
            }

            let min_x = cmp::min(v[0].x, cmp::min(v[1].x, v[2].x));
            let min_y = cmp::min(v[0].y, cmp::min(v[1].y, v[2].y));

            let max_x = cmp::max(v[0].x, cmp::max(v[1].x, v[2].x));
            let max_y = cmp::max(v[0].y, cmp::max(v[1].y, v[2].y));

            if (max_x >= 1024 && min_x >= 1024) || (max_x < 0 && min_x < 0) {
                return;
            }

            if (max_y >= 512 && min_y >= 512) || (max_y < 0 && min_y < 0) {
                return;
            }

            if (max_x - min_x) >= 1024 || (max_y - min_y) >= 512 {
                return;
            }
        }

        for i in 0..polygon.vertices.len() {
            let vertex = &polygon.vertices[i];

            let u = vertex.u;
            let v = vertex.v;

            let gl_vert = &mut vertices[i];

            gl_vert.orig[0] = vertex.x as f32;
            gl_vert.orig[1] = vertex.y as f32;

            gl_vert.position[0] = (vertex.x as f32 / VRAM_WIDTH as f32) * 2.0 - 1.0;
            gl_vert.position[1] = 1.0 - (vertex.y as f32 / VRAM_HEIGHT as f32) * 2.0;

            gl_vert.color[0] = vertex.color.r as f32 / 255.0;
            gl_vert.color[1] = vertex.color.g as f32 / 255.0;
            gl_vert.color[2] = vertex.color.b as f32 / 255.0;
            gl_vert.color[3] = 1.0;

            let u_f32 = u as f32;
            let v_f32 = v as f32;

            gl_vert.uv[0] = u_f32;
            gl_vert.uv[1] = v_f32;
            if let Some(texpage) = polygon.texpage {
                fragment_uniform.page = [texpage.x_base as u32 * 64, texpage.y_base1 as u32 * 16];
            }
        }

        let vertices_bytes: &[u8] = cast_slice(&vertices);
        let float_view = Float32Array::from(cast_slice::<u8, f32>(vertices_bytes));

        self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.vertex_buffer));
        self.gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &float_view,
            WebGl2RenderingContext::DYNAMIC_DRAW
        );

        let stride = std::mem::size_of::<GlVertex>() as i32;
        self.gl.vertex_attrib_pointer_with_i32(
            0,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            0
        );
        self.gl.enable_vertex_attrib_array(0);

        self.gl.vertex_attrib_pointer_with_i32(
            1,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            8
        );
        self.gl.enable_vertex_attrib_array(1);

        self.gl.vertex_attrib_pointer_with_i32(
            2,
            4,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            16
        );
        self.gl.enable_vertex_attrib_array(2);

        self.gl.vertex_attrib_pointer_with_i32(
            3,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            32
        );

        self.gl.enable_vertex_attrib_array(3);

        self.gl.viewport(0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_read));

        let location = self.gl.get_uniform_location(&self.program, "vramRead");
        self.gl.uniform1i(location.as_ref(), 0);

        self.gl.use_program(Some(&self.program));

        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.fbo_write));

        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, vertices.len() as i32);
    }

    pub fn present(&self, gpu: &mut GPU) {
        let (width, height) = gpu.get_dimensions();

        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.canvas.set_attribute("width", &format!("{width}")).unwrap();
        self.canvas.set_attribute("height", &format!("{height}")).unwrap();
        self.gl.viewport(0, 0, width as i32, height as i32);

        let loc_depth = self.gl.get_uniform_location(&self.fb_program, "displayDepth");
        let loc_start = self.gl.get_uniform_location(&self.fb_program, "displayStart");
        let loc_size = self.gl.get_uniform_location(&self.fb_program, "displaySize");

        self.gl.use_program(Some(&self.fb_program));

        self.gl.uniform1ui(loc_depth.as_ref(), gpu.display_depth as u32);
        self.gl.uniform2ui(loc_start.as_ref(), gpu.display_start_x, gpu.display_start_y);
        self.gl.uniform2ui(loc_size.as_ref(), width, height);

        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_write));

        let location = self.gl.get_uniform_location(&self.fb_program, "vramWrite");

        self.gl.uniform1i(location.as_ref(), 0);

        let vertices_bytes: &[u8] = cast_slice(&QUAD_VERTS);
        let float_view = Float32Array::from(cast_slice::<u8, f32>(vertices_bytes));

        self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.quad_buffer));
        self.gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &float_view,
            WebGl2RenderingContext::DYNAMIC_DRAW
        );

        let quad_stride = 16; // 4 floats * 4 bytes each

        self.gl.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, quad_stride, 0);
        self.gl.enable_vertex_attrib_array(0);

        self.gl.vertex_attrib_pointer_with_i32(1, 2, WebGl2RenderingContext::FLOAT, false, quad_stride, 8);
        self.gl.enable_vertex_attrib_array(1);

        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, 6);

    }

    fn update_texture_for_sampling(&self) {

    }

    fn execute_fill_vram(&self, params: FillVramParams) {

    }

    fn execute_vram_to_vram(&self, params: VramToVramTransferParams) {

    }
}