use std::cmp;

use bytemuck::{Pod, Zeroable, cast_slice};
use js_sys::{Float32Array, Uint16Array, wasm_bindgen::JsCast};
use rsx_redux::cpu::bus::gpu::{
    CPUTransferParams, DisplayDepth, FillVramParams, GPU, GPUCommand, Polygon, TexturePageColors,
    VRAM_HEIGHT, VRAM_WIDTH, VRamTransferParams, VramToVramTransferParams,
};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext, WebGlBuffer, WebGlContextAttributes,
    WebGlFramebuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation, window,
};

const QUAD_VERTS: [f32; 24] = [
    // pos        uv
    -1.0, -1.0,   0.0, 0.0,
     1.0, -1.0,   1.0, 0.0,
    -1.0,  1.0,   0.0, 1.0,
    -1.0,  1.0,   0.0, 1.0,
     1.0, -1.0,   1.0, 0.0,
     1.0,  1.0,   1.0, 1.0,
];

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

pub struct Renderer {
    canvas: HtmlCanvasElement,
    gl: WebGl2RenderingContext,
    vram_read: WebGlTexture,
    vram_write: WebGlTexture,
    program: WebGlProgram,
    vertex_buffer: WebGlBuffer,
    quad_buffer: WebGlBuffer,
    fbo_write: WebGlFramebuffer,
    fbo_read: WebGlFramebuffer,
    writeback_program: WebGlProgram,
    fb_program: WebGlProgram,
    location: Option<WebGlUniformLocation>,
    loc_has_texture: Option<WebGlUniformLocation>,
    loc_semitransparent: Option<WebGlUniformLocation>,
    loc_modulate: Option<WebGlUniformLocation>,
    loc_texture_mask_x: Option<WebGlUniformLocation>,
    loc_texture_mask_y: Option<WebGlUniformLocation>,
    loc_texture_offset_x: Option<WebGlUniformLocation>,
    loc_texture_offset_y: Option<WebGlUniformLocation>,
    loc_depth: Option<WebGlUniformLocation>,
    loc_transparent_mode: Option<WebGlUniformLocation>,
    loc_page: Option<WebGlUniformLocation>,
    loc_clut: Option<WebGlUniformLocation>,
    loc_force_mask_bit: Option<WebGlUniformLocation>,
    loc_preserve_masked_pixels: Option<WebGlUniformLocation>,
}

impl Renderer {
    pub fn new(canvas_id: &str) -> Self {
        let document = window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        let context_options = WebGlContextAttributes::new();
        context_options.set_alpha(false);
        context_options.set_preserve_drawing_buffer(true);

        let gl = canvas
            .get_context_with_context_options("webgl2", &context_options)
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

        // reuse the vertex shader for framebuffer for the writeback program
        let writeback_vert_shader_str = include_str!("shaders/vertex_fb.glsl");
        // create a new fragment shader just for writeback
        let writeback_frag_shader_str = include_str!("shaders/fragment_writeback.glsl");

        let fragment_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            fragment_shader_str,
        )
        .unwrap();
        let vertex_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::VERTEX_SHADER,
            vertex_shader_str,
        )
        .unwrap();
        let fb_frag_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            fb_frag_shader_str,
        )
        .unwrap();
        let fb_vert_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::VERTEX_SHADER,
            fb_vert_shader_str,
        )
        .unwrap();

        let writeback_frag_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            writeback_frag_shader_str,
        )
        .unwrap();
        let writeback_vert_shader = Self::compile_shader(
            &gl,
            WebGl2RenderingContext::VERTEX_SHADER,
            writeback_vert_shader_str,
        )
        .unwrap();

        let program = Self::link_program(&gl, &vertex_shader, &fragment_shader).unwrap();
        let fb_program = Self::link_program(&gl, &fb_vert_shader, &fb_frag_shader).unwrap();
        let writeback_program =
            Self::link_program(&gl, &writeback_vert_shader, &writeback_frag_shader).unwrap();

        let location = gl.get_uniform_location(&program, "vramRead");

        let loc_has_texture = gl.get_uniform_location(&program, "hasTexture");
        let loc_semitransparent = gl.get_uniform_location(&program, "semitransparent");
        let loc_modulate = gl.get_uniform_location(&program, "modulate");
        let loc_texture_mask_x = gl.get_uniform_location(&program, "textureMaskX");
        let loc_texture_mask_y = gl.get_uniform_location(&program, "textureMaskY");
        let loc_texture_offset_x = gl.get_uniform_location(&program, "textureOffsetX");
        let loc_texture_offset_y = gl.get_uniform_location(&program, "textureOffsetY");
        let loc_depth = gl.get_uniform_location(&program, "depth");
        let loc_transparent_mode = gl.get_uniform_location(&program, "transparentMode");
        let loc_page = gl.get_uniform_location(&program, "page");
        let loc_clut = gl.get_uniform_location(&program, "clut");
        let loc_force_mask_bit = gl.get_uniform_location(&program, "forceMaskBit");
        let loc_preserve_masked_pixels = gl.get_uniform_location(&program, "preserveMaskedPixels");

        let vertex_buffer = gl.create_buffer().unwrap();
        let quad_buffer = gl.create_buffer().unwrap();

        let fbo_write = gl.create_framebuffer().unwrap();
        let fbo_read = gl.create_framebuffer().unwrap();

        Self::bind_texture_to_framebuffer(
            &gl,
            WebGl2RenderingContext::TEXTURE0,
            WebGl2RenderingContext::RGBA8,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
            &vram_write,
            &fbo_write
        );

        Self::bind_texture_to_framebuffer(
            &gl,
            WebGl2RenderingContext::TEXTURE1,
            WebGl2RenderingContext::R16UI,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
            &vram_read,
            &fbo_read
        );

        let float_view = Float32Array::from(QUAD_VERTS.as_slice());

        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&quad_buffer));
        gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &float_view,
            WebGl2RenderingContext::DYNAMIC_DRAW,
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
            writeback_program,
            fbo_read,
            location,
            loc_has_texture,
            loc_semitransparent,
            loc_modulate,
            loc_texture_mask_x,
            loc_texture_mask_y,
            loc_texture_offset_x,
            loc_texture_offset_y,
            loc_depth,
            loc_transparent_mode,
            loc_page,
            loc_clut,
            loc_force_mask_bit,
            loc_preserve_masked_pixels,
        }
    }

    fn bind_texture_to_framebuffer(
        gl: &WebGl2RenderingContext,
        active_texture: u32,
        internal_format: u32,
        width: i32,
        height: i32,
        texture: &WebGlTexture,
        framebuffer: &WebGlFramebuffer,
    ) {
        gl.active_texture(active_texture);
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(texture));

        gl.tex_storage_2d(
            WebGl2RenderingContext::TEXTURE_2D,
            1,
            internal_format,
            width,
            height,
        );

        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );

        gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            WebGl2RenderingContext::FRAMEBUFFER,
            WebGl2RenderingContext::COLOR_ATTACHMENT0,
            WebGl2RenderingContext::TEXTURE_2D,
            Some(texture),
            0,
        );
    }


    fn compile_shader(
        gl: &WebGl2RenderingContext,
        shader_type: u32,
        source: &str,
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
                .unwrap_or("unknown error creating shader".to_string()))
        }
    }

    fn link_program(
        gl: &WebGl2RenderingContext,
        vertex_shader: &WebGlShader,
        fragment_shader: &WebGlShader,
    ) -> Result<WebGlProgram, String> {
        let program = gl.create_program().ok_or("Unable to create gl program")?;

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
                .unwrap_or("Unable to create gl program for unknown reason".to_string()))
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
        if gpu.display_depth == DisplayDepth::Bit24 {
            self.vram_writeback(None, Some(&gpu));
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
                    // TODO: maybe do a vram writeback if needed
                    self.execute_vram_to_vram(params);
                }
                GPUCommand::FillVRAM(params) => {
                    self.execute_fill_vram(params);
                }
                GPUCommand::RenderPolygon(polygon) => {
                    let is_16bpp = polygon.textured
                        && polygon.texpage.map(|texpage| texpage.texture_page_colors)
                            == Some(TexturePageColors::Bit15);
                    if polygon.semitransparent || is_16bpp {
                        self.vram_writeback(Some(&polygon), None);
                    }

                    self.render_polygon(&polygon);
                }
            }
        }
    }

    fn get_drawing_area(polygon: &Polygon, invert: bool) -> (u32, u32, u32, u32) {
        let y = if invert {
            VRAM_HEIGHT as u32 - polygon.y2 - 1
        } else {
            polygon.y1 as u32
        };
        (
            polygon.x1,
            y,
            polygon.x2 - polygon.x1 + 1,
            polygon.y2 - polygon.y1 + 1,
        )
    }

    fn vram_writeback(&self, polygon: Option<&Polygon>, gpu: Option<&GPU>) {
        let (start_x, start_y, width, height) = if let Some(polygon) = polygon {
            Self::get_drawing_area(polygon, false)
        } else if let Some(gpu) = gpu {
            let (width, height) = gpu.get_dimensions();
            let writeback_width = (width * 3 + 1) / 2;
            (
                gpu.display_start_x,
                gpu.display_start_y,
                writeback_width,
                height,
            )
        } else {
            panic!("no gpu or polygon passed to vram_writeback");
        };

        self.gl
            .bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.fbo_read));

        self.gl.use_program(Some(&self.writeback_program));
        self.gl
            .viewport(0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_write));

        let loc = self
            .gl
            .get_uniform_location(&self.writeback_program, "vramWrite");

        self.gl.uniform1i(loc.as_ref(), 0);

        self.bind_quad_verts();

        self.gl.enable(WebGl2RenderingContext::SCISSOR_TEST);
        self.gl
            .scissor(start_x as i32, start_y as i32, width as i32, height as i32);

        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, 6);
        self.gl.disable(WebGl2RenderingContext::SCISSOR_TEST);
    }

    fn bind_quad_verts(&self) {
        self.gl.bind_buffer(
            WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.quad_buffer),
        );

        let quad_stride = 16; // 4 floats * 4 bytes each

        self.gl.vertex_attrib_pointer_with_i32(
            0,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            quad_stride,
            0,
        );
        self.gl.enable_vertex_attrib_array(0);

        self.gl.vertex_attrib_pointer_with_i32(
            1,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            quad_stride,
            8,
        );
        self.gl.enable_vertex_attrib_array(1);
    }

    fn execute_cpu_to_vram(&self, params: VRamTransferParams) {
        let mut rgba8_buffer: Vec<u8> = Vec::new();

        let mut i = 0;
        for _ in 0..params.height {
            for _ in 0..params.width {
                let halfword = params.halfwords[i];

                let mut r = halfword & 0x1f;
                let mut g = (halfword >> 5) & 0x1f;
                let mut b = (halfword >> 10) & 0x1f;
                let a = ((halfword >> 15) & 1) * 0xff;

                r = r << 3 | r >> 2;
                g = g << 3 | g >> 2;
                b = b << 3 | b >> 2;

                rgba8_buffer.push(r as u8);
                rgba8_buffer.push(g as u8);
                rgba8_buffer.push(b as u8);
                rgba8_buffer.push(a as u8);

                i += 1;
            }
        }

        self.transfer_bytes_to_textures(
            params.start_x as i32,
            params.start_y as i32,
            params.width as i32,
            params.height as i32,
            &rgba8_buffer,
            &params.halfwords,
            true,
        );
    }

    fn handle_cpu_transfer(&self, params: CPUTransferParams) -> Vec<u16> {
        let mut rgba8_buf = vec![0u8; params.width as usize * params.height as usize * 4];

        self.gl
            .bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.fbo_write));
        self.gl
            .read_pixels_with_opt_u8_array(
                params.start_x as i32,
                VRAM_HEIGHT as i32 - params.height as i32 - params.start_y as i32,
                params.width as i32,
                params.height as i32,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(&mut rgba8_buf),
            )
            .unwrap();

        let mut halfwords = Vec::new();

        for y in (0..params.height as usize).rev() {
            for x in (0..params.width as usize).step_by(4) {
                let index = (x + y * params.width as usize) * 4;
                let r = (rgba8_buf[index] >> 3) as u16;
                let g = (rgba8_buf[index + 1] >> 3) as u16;
                let b = (rgba8_buf[index + 2] >> 3) as u16;
                let a = (rgba8_buf[index + 3] != 0) as u16;

                let halfword = r | g << 5 | b << 10 | a << 15;

                halfwords.push(halfword);

            }
        }

        halfwords
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

        self.gl.bind_buffer(
            WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.vertex_buffer),
        );
        self.gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &float_view,
            WebGl2RenderingContext::DYNAMIC_DRAW,
        );

        let stride = std::mem::size_of::<GlVertex>() as i32;
        self.gl.vertex_attrib_pointer_with_i32(
            0,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            0,
        );
        self.gl.enable_vertex_attrib_array(0);

        self.gl.vertex_attrib_pointer_with_i32(
            1,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            8,
        );
        self.gl.enable_vertex_attrib_array(1);

        self.gl.vertex_attrib_pointer_with_i32(
            2,
            4,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            16,
        );
        self.gl.enable_vertex_attrib_array(2);

        self.gl.vertex_attrib_pointer_with_i32(
            3,
            2,
            WebGl2RenderingContext::FLOAT,
            false,
            stride,
            32,
        );

        self.gl.enable_vertex_attrib_array(3);

        self.gl
            .viewport(0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_read));

        self.gl.use_program(Some(&self.program));

        self.gl.uniform1i(self.location.as_ref(), 0);
        self.gl.uniform1i(
            self.loc_has_texture.as_ref(),
            fragment_uniform.has_texture as i32,
        );
        self.gl.uniform1i(
            self.loc_semitransparent.as_ref(),
            fragment_uniform.semitransparent as i32,
        );
        self.gl
            .uniform1i(self.loc_modulate.as_ref(), fragment_uniform.modulate as i32);
        self.gl.uniform1i(
            self.loc_force_mask_bit.as_ref(),
            fragment_uniform.force_mask_bit as i32,
        );
        self.gl.uniform1i(
            self.loc_preserve_masked_pixels.as_ref(),
            fragment_uniform.preserve_masked_pixels as i32,
        );

        self.gl.uniform1ui(
            self.loc_texture_mask_x.as_ref(),
            fragment_uniform.texture_mask_x,
        );
        self.gl.uniform1ui(
            self.loc_texture_mask_y.as_ref(),
            fragment_uniform.texture_mask_y,
        );
        self.gl.uniform1ui(
            self.loc_texture_offset_x.as_ref(),
            fragment_uniform.texture_offset_x,
        );
        self.gl.uniform1ui(
            self.loc_texture_offset_y.as_ref(),
            fragment_uniform.texture_offset_y,
        );
        self.gl.uniform1ui(
            self.loc_transparent_mode.as_ref(),
            fragment_uniform.transparent_mode,
        );

        self.gl.uniform2ui(
            self.loc_page.as_ref(),
            fragment_uniform.page[0],
            fragment_uniform.page[1],
        );

        self.gl.uniform2ui(
            self.loc_clut.as_ref(),
            fragment_uniform.clut[0],
            fragment_uniform.clut[1],
        );

        self.gl
            .uniform1i(self.loc_depth.as_ref(), fragment_uniform.depth);

        self.gl
            .bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.fbo_write));

        let (start_x, start_y, width, height) = Self::get_drawing_area(polygon, true);

        self.gl.enable(WebGl2RenderingContext::SCISSOR_TEST);
        self.gl
            .scissor(start_x as i32, start_y as i32, width as i32, height as i32);

        self.gl
            .draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, vertices.len() as i32);

        self.gl.disable(WebGl2RenderingContext::SCISSOR_TEST);
    }

    pub fn present(&self, gpu: &mut GPU) {
        let (width, height) = gpu.get_dimensions();

        // self.canvas.set_width(width);
        // self.canvas.set_height(height);
        self.canvas
            .set_attribute("width", &format!("{width}"))
            .unwrap();
        self.canvas
            .set_attribute("height", &format!("{height}"))
            .unwrap();
        self.gl.viewport(0, 0, width as i32, height as i32);

        let loc_depth = self
            .gl
            .get_uniform_location(&self.fb_program, "displayDepth");
        let loc_start = self
            .gl
            .get_uniform_location(&self.fb_program, "displayStart");
        let loc_size = self
            .gl
            .get_uniform_location(&self.fb_program, "displaySize");

        self.gl.use_program(Some(&self.fb_program));

        self.gl
            .uniform1ui(loc_depth.as_ref(), gpu.display_depth as u32);
        self.gl
            .uniform2ui(loc_start.as_ref(), gpu.display_start_x, gpu.display_start_y);
        self.gl.uniform2ui(loc_size.as_ref(), width, height);

        self.gl
            .bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_write));

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE1);
        self.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_read));

        let loc_write = self.gl.get_uniform_location(&self.fb_program, "vramWrite");
        let loc_read = self.gl.get_uniform_location(&self.fb_program, "vramRead");

        self.gl.uniform1i(loc_write.as_ref(), 0);
        self.gl.uniform1i(loc_read.as_ref(), 1);

        self.bind_quad_verts();

        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, 6);
    }

    fn execute_fill_vram(&self, params: FillVramParams) {
        let mut rgba8_bytes: Vec<u8> = Vec::new();
        let mut halfwords: Vec<u16> = Vec::new();

        let mut r = (params.pixel & 0x1f) as u8;
        let mut g = ((params.pixel >> 5) & 0x1f) as u8;
        let mut b = ((params.pixel >> 10) & 0x1f) as u8;

        r = r << 3 | r >> 2;
        g = g << 3 | g >> 2;
        b = b << 3 | b >> 2;

        let a = 0;

        for _ in 0..params.height {
            for _ in 0..params.width {
                rgba8_bytes.push(r);
                rgba8_bytes.push(g);
                rgba8_bytes.push(b);
                rgba8_bytes.push(a);

                halfwords.push(params.pixel);
            }
        }

        self.transfer_bytes_to_textures(
            params.start_x as i32,
            params.start_y as i32,
            params.width as i32,
            params.height as i32,
            &rgba8_bytes,
            &halfwords,
            true,
        );
    }

    fn transfer_bytes_to_textures(
        &self,
        start_x: i32,
        start_y: i32,
        width: i32,
        height: i32,
        rgba8_bytes: &[u8],
        halfwords: &[u16],
        invert: bool,
    ) {
        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_write));

        if invert {
            self.gl
                .pixel_storei(WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 1);
        }
        self.gl
            .pixel_storei(WebGl2RenderingContext::UNPACK_ALIGNMENT, 4);

        let y_offset = if invert {
            VRAM_HEIGHT as i32 - start_y - height
        } else {
            start_y
        };

        self.gl
            .tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_u8_array(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                start_x,
                y_offset,
                width,
                height,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(rgba8_bytes),
            )
            .unwrap();

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE1);
        self.gl
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&self.vram_read));
        self.gl
            .pixel_storei(WebGl2RenderingContext::UNPACK_ALIGNMENT, 2);
        self.gl
            .pixel_storei(WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 0);

        let js_array = Uint16Array::from(halfwords);

        self.gl.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_array_buffer_view_and_src_offset(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            start_x,
            start_y,
            width,
            height,
            WebGl2RenderingContext::RED_INTEGER,
            WebGl2RenderingContext::UNSIGNED_SHORT,
            &js_array,
            0,
        ).unwrap();
    }

    fn execute_vram_to_vram(&self, params: VramToVramTransferParams) {

        let temp_rgba_texture = self.gl.create_texture().unwrap();
        let temp_rgba_fbo = self.gl.create_framebuffer().unwrap();

        Self::bind_texture_to_framebuffer(
            &self.gl,
            WebGl2RenderingContext::TEXTURE2,
            WebGl2RenderingContext::RGBA8,
            params.width as i32,
            params.height as i32,
            &temp_rgba_texture,
            &temp_rgba_fbo
        );

        self.gl.bind_framebuffer(WebGl2RenderingContext::READ_FRAMEBUFFER, Some(&self.fbo_write));
        self.gl.bind_framebuffer(WebGl2RenderingContext::DRAW_FRAMEBUFFER, Some(&temp_rgba_fbo));

        let source_y_flipped = (VRAM_HEIGHT as u32 - params.source_start_y - params.height) as i32;
        let destination_y_flipped = (VRAM_HEIGHT as u32 - params.destination_start_y - params.height) as i32;

        self.gl.blit_framebuffer(
            params.source_start_x as i32,
            source_y_flipped,
            params.source_start_x as i32 + params.width as i32,
            source_y_flipped + params.height as i32,
            0,
            0,
            params.width as i32,
            params.height as i32,
            WebGl2RenderingContext::COLOR_BUFFER_BIT,
            WebGl2RenderingContext::NEAREST
        );

        self.gl.bind_framebuffer(WebGl2RenderingContext::READ_FRAMEBUFFER, Some(&temp_rgba_fbo));
        self.gl.bind_framebuffer(WebGl2RenderingContext::DRAW_FRAMEBUFFER, Some(&self.fbo_write));

        self.gl.blit_framebuffer(
            0,
            0,
            params.width as i32,
            params.height as i32,
            params.destination_start_x as i32,
            destination_y_flipped,
            params.destination_start_x as i32 + params.width as i32,
            destination_y_flipped + params.height as i32,
            WebGl2RenderingContext::COLOR_BUFFER_BIT,
            WebGl2RenderingContext::NEAREST
        );

        let temp_r16_texture = self.gl.create_texture().unwrap();
        let temp_r16_fbo = self.gl.create_framebuffer().unwrap();

        Self::bind_texture_to_framebuffer(
            &self.gl,
            WebGl2RenderingContext::TEXTURE3,
            WebGl2RenderingContext::R16UI,
            params.width as i32,
            params.height as i32,
            &temp_r16_texture,
            &temp_r16_fbo,
        );

        self.gl.bind_framebuffer(WebGl2RenderingContext::READ_FRAMEBUFFER, Some(&self.fbo_read));
        self.gl.bind_framebuffer(WebGl2RenderingContext::DRAW_FRAMEBUFFER, Some(&temp_r16_fbo));

        self.gl.blit_framebuffer(
            params.source_start_x as i32,
            params.source_start_y as i32,
            params.source_start_x as i32 + params.width as i32,
            params.source_start_y as i32 + params.height as i32,
            0,
            0,
            params.width as i32,
            params.height as i32,
            WebGl2RenderingContext::COLOR_BUFFER_BIT,
            WebGl2RenderingContext::NEAREST
        );

        self.gl.bind_framebuffer(WebGl2RenderingContext::READ_FRAMEBUFFER, Some(&temp_r16_fbo));
        self.gl.bind_framebuffer(WebGl2RenderingContext::DRAW_FRAMEBUFFER, Some(&self.fbo_read));

        self.gl.blit_framebuffer(
            0,
            0,
            params.width as i32,
            params.height as i32,
            params.destination_start_x as i32,
            params.destination_start_y as i32,
            params.destination_start_x as i32 + params.width as i32,
            params.destination_start_y as i32 + params.height as i32,
            WebGl2RenderingContext::COLOR_BUFFER_BIT,
            WebGl2RenderingContext::NEAREST
        );

        self.gl.delete_texture(Some(&temp_rgba_texture));
        self.gl.delete_framebuffer(Some(&temp_rgba_fbo));

        self.gl.delete_texture(Some(&temp_r16_texture));
        self.gl.delete_framebuffer(Some(&temp_r16_fbo));
    }

    pub fn get_vram_textures(&self) -> (Vec<u8>, Vec<u8>) {
        let mut rgba8_buf = vec![0u8; VRAM_WIDTH * VRAM_HEIGHT * 4];
        let mut rgba16_buf = vec![0u16; VRAM_WIDTH * VRAM_HEIGHT];

        self.gl
            .bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.fbo_write));
        self.gl
            .read_pixels_with_opt_u8_array(
                0,
                0,
                VRAM_WIDTH as i32,
                VRAM_HEIGHT as i32,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(&mut rgba8_buf),
            )
            .unwrap();

        self.gl
            .bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.fbo_read));
        {
            let js_array =
                unsafe { Uint16Array::view_mut_raw(rgba16_buf.as_mut_ptr(), rgba16_buf.len()) };

            self.gl
                .read_pixels_with_opt_array_buffer_view(
                    0,
                    0,
                    VRAM_WIDTH as i32,
                    VRAM_HEIGHT as i32,
                    WebGl2RenderingContext::RED_INTEGER,
                    WebGl2RenderingContext::UNSIGNED_SHORT,
                    Some(&js_array),
                )
                .unwrap();
        }

        let rgba16_bytes: &[u8] = cast_slice(&rgba16_buf);

        (rgba8_buf, rgba16_bytes.to_vec())
    }

    pub fn set_vram_textures(&self, rgba8_buf: Vec<u8>, rgba16_buf: Vec<u8>) {
        let rgba16_buf: &[u16] = cast_slice(&rgba16_buf);

        self.transfer_bytes_to_textures(
            0,
            0,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
            &rgba8_buf,
            rgba16_buf,
            false,
        );
    }
}
