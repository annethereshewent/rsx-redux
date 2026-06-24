use std::cmp;

use bytemuck::{cast_slice, cast_slice_mut, Pod, Zeroable};
use glow::{Context, HasContext, NativeBuffer, NativeFramebuffer, NativeProgram, NativeShader, NativeTexture, NativeUniformLocation, NativeVertexArray, PixelPackData, PixelUnpackData};
use rsx_redux::cpu::bus::gpu::{
    CPUTransferParams, DisplayDepth, FillVramParams, GPU, GPUCommand, Polygon, TexturePageColors,
    VRAM_HEIGHT, VRAM_WIDTH, VRamTransferParams, VramToVramTransferParams,
};
use sdl2::{video::{GLContext, Window}};

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
    gl: Context,
    _gl_context: GLContext,
    vram_read: NativeTexture,
    vram_write: NativeTexture,
    program: NativeProgram,
    vertex_buffer: NativeBuffer,
    quad_buffer: NativeBuffer,
    fbo_write: NativeFramebuffer,
    fbo_read: NativeFramebuffer,
    writeback_program: NativeProgram,
    fb_program: NativeProgram,
    location: Option<NativeUniformLocation>,
    loc_has_texture: Option<NativeUniformLocation>,
    loc_semitransparent: Option<NativeUniformLocation>,
    loc_modulate: Option<NativeUniformLocation>,
    loc_texture_mask_x: Option<NativeUniformLocation>,
    loc_texture_mask_y: Option<NativeUniformLocation>,
    loc_texture_offset_x: Option<NativeUniformLocation>,
    loc_texture_offset_y: Option<NativeUniformLocation>,
    loc_depth: Option<NativeUniformLocation>,
    loc_transparent_mode: Option<NativeUniformLocation>,
    loc_page: Option<NativeUniformLocation>,
    loc_clut: Option<NativeUniformLocation>,
    loc_force_mask_bit: Option<NativeUniformLocation>,
    loc_preserve_masked_pixels: Option<NativeUniformLocation>,
    quad_vao: NativeVertexArray,
}

impl Renderer {
    fn glow_context(window: &Window) -> Context {
        unsafe {
            Context::from_loader_function(|s| window.subsystem().gl_get_proc_address(s) as _)
        }
    }
    pub fn new(window: &Window, gl_context: GLContext) -> Self {
        let gl = Self::glow_context(window);

        let vram_read = unsafe { gl.create_texture().unwrap() };
        let vram_write = unsafe { gl.create_texture().unwrap() };

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
            glow::FRAGMENT_SHADER,
            fragment_shader_str,
        )
        .unwrap();
        let vertex_shader = Self::compile_shader(
            &gl,
            glow::VERTEX_SHADER,
            vertex_shader_str,
        )
        .unwrap();
        let fb_frag_shader = Self::compile_shader(
            &gl,
            glow::FRAGMENT_SHADER,
            fb_frag_shader_str,
        )
        .unwrap();
        let fb_vert_shader = Self::compile_shader(
            &gl,
            glow::VERTEX_SHADER,
            fb_vert_shader_str,
        )
        .unwrap();

        let writeback_frag_shader = Self::compile_shader(
            &gl,
            glow::FRAGMENT_SHADER,
            writeback_frag_shader_str,
        )
        .unwrap();
        let writeback_vert_shader = Self::compile_shader(
            &gl,
            glow::VERTEX_SHADER,
            writeback_vert_shader_str,
        )
        .unwrap();

        let program = Self::link_program(&gl, vertex_shader, fragment_shader).unwrap();
        let fb_program = Self::link_program(&gl, fb_vert_shader, fb_frag_shader).unwrap();
        let writeback_program =
            Self::link_program(&gl, writeback_vert_shader, writeback_frag_shader).unwrap();

        let location = unsafe { gl.get_uniform_location(program, "vramRead") };

        let loc_has_texture = unsafe { gl.get_uniform_location(program, "hasTexture") };
        let loc_semitransparent = unsafe { gl.get_uniform_location(program, "semitransparent") };
        let loc_modulate = unsafe { gl.get_uniform_location(program, "modulate") };
        let loc_texture_mask_x = unsafe { gl.get_uniform_location(program, "textureMaskX") };
        let loc_texture_mask_y = unsafe { gl.get_uniform_location(program, "textureMaskY") };
        let loc_texture_offset_x = unsafe { gl.get_uniform_location(program, "textureOffsetX") };
        let loc_texture_offset_y = unsafe { gl.get_uniform_location(program, "textureOffsetY") };
        let loc_depth = unsafe { gl.get_uniform_location(program, "depth") };
        let loc_transparent_mode = unsafe { gl.get_uniform_location(program, "transparentMode") };
        let loc_page = unsafe { gl.get_uniform_location(program, "page") };
        let loc_clut = unsafe { gl.get_uniform_location(program, "clut") };
        let loc_force_mask_bit = unsafe { gl.get_uniform_location(program, "forceMaskBit") };
        let loc_preserve_masked_pixels = unsafe { gl.get_uniform_location(program, "preserveMaskedPixels") };

        let vertex_buffer = unsafe { gl.create_buffer().unwrap() };
        let quad_buffer = unsafe { gl.create_buffer().unwrap() };

        let fbo_write = unsafe { gl.create_framebuffer().unwrap() };
        let fbo_read = unsafe { gl.create_framebuffer().unwrap() };

        Self::bind_texture_to_framebuffer(
            &gl,
            glow::TEXTURE0,
            glow::RGBA8,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
            &vram_write,
            &fbo_write,
        );

        Self::bind_texture_to_framebuffer(
            &gl,
            glow::TEXTURE1,
            glow::R16UI,
            VRAM_WIDTH as i32,
            VRAM_HEIGHT as i32,
            &vram_read,
            &fbo_read,
        );

        // let float_view = Float32Array::from(QUAD_VERTS.as_slice());

        let slice: &[u8] = cast_slice(QUAD_VERTS.as_slice());

        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                slice,
                glow::DYNAMIC_DRAW,
            );
        }

        let quad_vao = unsafe { gl.create_vertex_array().unwrap() };
        unsafe { gl.bind_vertex_array(Some(quad_vao)); }

        Self {
            quad_vao,
            gl,
            _gl_context: gl_context,
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
        gl: &Context,
        active_texture: u32,
        internal_format: u32,
        width: i32,
        height: i32,
        texture: &NativeTexture,
        framebuffer: &NativeFramebuffer,
    ) {
        unsafe {
            gl.active_texture(active_texture);
            gl.bind_texture(glow::TEXTURE_2D, Some(*texture));

            gl.tex_storage_2d(
                glow::TEXTURE_2D,
                1,
                internal_format,
                width,
                height,
            );

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(*framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(*texture),
                0,
            );
        }
    }

    fn compile_shader(
        gl: &Context,
        shader_type: u32,
        source: &str,
    ) -> Result<NativeShader, String> {
        let shader = unsafe { gl
            .create_shader(shader_type)?
        };

        unsafe {
            gl.shader_source(shader, source);
            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                println!("shader compile error: {}", gl.get_shader_info_log(shader));
            }
        }

        Ok(shader)
    }

    fn link_program(
        gl: &Context,
        vertex_shader: NativeShader,
        fragment_shader: NativeShader,
    ) -> Result<NativeProgram, String> {
        let program = unsafe { gl.create_program()? };

        unsafe {
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                println!("program completion error: {}", gl.get_program_info_log(program));
            }
        }

        Ok(program)
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
        unsafe {
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
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
                    let is_15bpp = polygon.textured
                        && polygon.texpage.map(|texpage| texpage.texture_page_colors)
                            == Some(TexturePageColors::Bit15);
                    if polygon.semitransparent || is_15bpp || polygon.preserve_masked_pixels {
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

        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo_read));

            self.gl.use_program(Some(self.writeback_program));
            self.gl
                .viewport(0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.vram_write));

            let loc = self
                .gl
                .get_uniform_location(self.writeback_program, "vramWrite");

            self.gl.uniform_1_i32(loc.as_ref(), 0);

            self.bind_quad_verts();

            self.gl.enable(glow::SCISSOR_TEST);
            self.gl
                .scissor(start_x as i32, start_y as i32, width as i32, height as i32);

            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);
            self.gl.disable(glow::SCISSOR_TEST);
        }
    }

    fn bind_quad_verts(&self) {
        unsafe {
            self.gl.bind_vertex_array(Some(self.quad_vao));
            self.gl.bind_buffer(
                glow::ARRAY_BUFFER,
                Some(self.quad_buffer),
            );

            let quad_stride = 16; // 4 floats * 4 bytes each

            self.gl.vertex_attrib_pointer_f32(
                0,
                2,
                glow::FLOAT,
                false,
                quad_stride,
                0,

            );
            self.gl.enable_vertex_attrib_array(0);

            self.gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                quad_stride,
                8,
            );
            self.gl.enable_vertex_attrib_array(1);
        }
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

        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo_write));
            self.gl
                .read_pixels(
                    params.start_x as i32,
                    VRAM_HEIGHT as i32 - params.height as i32 - params.start_y as i32,
                    params.width as i32,
                    params.height as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    PixelPackData::Slice(Some(&mut rgba8_buf))
                );
        }

        let mut halfwords = Vec::new();

        for y in (0..params.height as usize).rev() {
            for x in 0..params.width as usize {
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

        unsafe {
            self.gl.bind_buffer(
                glow::ARRAY_BUFFER,
                Some(self.vertex_buffer),
            );
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                vertices_bytes,
                glow::DYNAMIC_DRAW,
            );

            let stride = std::mem::size_of::<GlVertex>() as i32;
            self.gl.vertex_attrib_pointer_f32(
                0,
                2,
                glow::FLOAT,
                false,
                stride,
                0
            );
            self.gl.enable_vertex_attrib_array(0);

            self.gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                stride,
                8,
            );
            self.gl.enable_vertex_attrib_array(1);

            self.gl.vertex_attrib_pointer_f32(
                2,
                4,
                glow::FLOAT,
                false,
                stride,
                16,
            );
            self.gl.enable_vertex_attrib_array(2);

            self.gl.vertex_attrib_pointer_f32(
                3,
                2,
                glow::FLOAT,
                false,
                stride,
                32,
            );

            self.gl.enable_vertex_attrib_array(3);

            self.gl
                .viewport(0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.vram_read));

            self.gl.use_program(Some(self.program));

            self.gl.uniform_1_i32(self.location.as_ref(), 0);
            self.gl.uniform_1_i32(
                self.loc_has_texture.as_ref(),
                fragment_uniform.has_texture as i32,
            );
            self.gl.uniform_1_i32(
                self.loc_semitransparent.as_ref(),
                fragment_uniform.semitransparent as i32,
            );
            self.gl
                .uniform_1_i32(self.loc_modulate.as_ref(), fragment_uniform.modulate as i32);
            self.gl.uniform_1_i32(
                self.loc_force_mask_bit.as_ref(),
                fragment_uniform.force_mask_bit as i32,
            );
            self.gl.uniform_1_i32(
                self.loc_preserve_masked_pixels.as_ref(),
                fragment_uniform.preserve_masked_pixels as i32,
            );

            self.gl.uniform_1_u32(
                self.loc_texture_mask_x.as_ref(),
                fragment_uniform.texture_mask_x,
            );
            self.gl.uniform_1_u32(
                self.loc_texture_mask_y.as_ref(),
                fragment_uniform.texture_mask_y,
            );
            self.gl.uniform_1_u32(
                self.loc_texture_offset_x.as_ref(),
                fragment_uniform.texture_offset_x,
            );
            self.gl.uniform_1_u32(
                self.loc_texture_offset_y.as_ref(),
                fragment_uniform.texture_offset_y,
            );
            self.gl.uniform_1_u32(
                self.loc_transparent_mode.as_ref(),
                fragment_uniform.transparent_mode,
            );

            self.gl.uniform_2_u32(
                self.loc_page.as_ref(),
                fragment_uniform.page[0],
                fragment_uniform.page[1],
            );

            self.gl.uniform_2_u32(
                self.loc_clut.as_ref(),
                fragment_uniform.clut[0],
                fragment_uniform.clut[1],
            );

            self.gl
                .uniform_1_i32(self.loc_depth.as_ref(), fragment_uniform.depth);

            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo_write));

            let (start_x, start_y, width, height) = Self::get_drawing_area(polygon, true);

            self.gl.enable(glow::SCISSOR_TEST);
            self.gl
                .scissor(start_x as i32, start_y as i32, width as i32, height as i32);

            let primitive_type = if polygon.is_line {
                glow::LINES
            } else {
                glow::TRIANGLES
            };

            self.gl
                .draw_arrays(primitive_type, 0, vertices.len() as i32);

            self.gl.disable(glow::SCISSOR_TEST);
        }
    }

    pub fn present(&self, gpu: &mut GPU) {
        let (width, height) = gpu.get_dimensions();

        // self.canvas
        //     .set_attribute("width", &format!("{width}"))
        //     .unwrap();
        // self.canvas
        //     .set_attribute("height", &format!("{height}"))
        //     .unwrap();

        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);

            let loc_depth = self
                .gl
                .get_uniform_location(self.fb_program, "displayDepth");
            let loc_start = self
                .gl
                .get_uniform_location(self.fb_program, "displayStart");
            let loc_size = self
                .gl
                .get_uniform_location(self.fb_program, "displaySize");

            self.gl.use_program(Some(self.fb_program));

            self.gl
                .uniform_1_u32(loc_depth.as_ref(), gpu.display_depth as u32);
            self.gl
                .uniform_2_u32(loc_start.as_ref(), gpu.display_start_x, gpu.display_start_y);
            self.gl.uniform_2_u32(loc_size.as_ref(), width, height);

            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, None);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.vram_write));

            self.gl.active_texture(glow::TEXTURE1);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.vram_read));

            let loc_write = self.gl.get_uniform_location(self.fb_program, "vramWrite");
            let loc_read = self.gl.get_uniform_location(self.fb_program, "vramRead");

            self.gl.uniform_1_i32(loc_write.as_ref(), 0);
            self.gl.uniform_1_i32(loc_read.as_ref(), 1);

            self.bind_quad_verts();

            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);
            }
        }
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
        unsafe {
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.vram_write));

            // if invert {
            //     self.gl
            //         .pixel_store_i32(glow::UNPACK_FLIP_Y_WEBGL, 1);
            // }

            self.gl
                .pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);

            let y_offset = if invert {
                VRAM_HEIGHT as i32 - start_y - height
            } else {
                start_y
            };

            self.gl
                .tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    start_x,
                    y_offset,
                    width,
                    height,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    PixelUnpackData::Slice(Some(rgba8_bytes))
                );

            self.gl.active_texture(glow::TEXTURE1);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.vram_read));
            self.gl
                .pixel_store_i32(glow::UNPACK_ALIGNMENT, 2);

            // let js_array = Uint16Array::from(halfwords);

            let slice: &[u8] = cast_slice(halfwords);

            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                start_x,
                start_y,
                width,
                height,
                glow::RED_INTEGER,
                glow::UNSIGNED_SHORT,
                PixelUnpackData::Slice(Some(slice))
            );
        }
    }

    fn execute_vram_to_vram(&self, params: VramToVramTransferParams) {
        if params.destination_start_x == params.source_start_x
            && params.destination_start_y == params.source_start_y
        {
            // for some reason the PS1 will transfer from the same source to same destination as some sort of NOP,
            // and openGL will break things if it attempts to do this blit, so it's best to explicitly return if that's the case
            return;
        }

        unsafe {
            let temp_rgba_texture = self.gl.create_texture().unwrap();
            let temp_rgba_fbo = self.gl.create_framebuffer().unwrap();

            Self::bind_texture_to_framebuffer(
                &self.gl,
                glow::TEXTURE2,
                glow::RGBA8,
                params.width as i32,
                params.height as i32,
                &temp_rgba_texture,
                &temp_rgba_fbo,
            );

            self.gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(self.fbo_write),
            );
            self.gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                Some(temp_rgba_fbo),
            );

            let source_y_flipped = (VRAM_HEIGHT as u32 - params.source_start_y - params.height) as i32;
            let destination_y_flipped =
                (VRAM_HEIGHT as u32 - params.destination_start_y - params.height) as i32;

            self.gl.blit_framebuffer(
                params.source_start_x as i32,
                source_y_flipped,
                params.source_start_x as i32 + params.width as i32,
                source_y_flipped + params.height as i32,
                0,
                0,
                params.width as i32,
                params.height as i32,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );

            self.gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(temp_rgba_fbo),
            );
            self.gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                Some(self.fbo_write),
            );

            self.gl.blit_framebuffer(
                0,
                0,
                params.width as i32,
                params.height as i32,
                params.destination_start_x as i32,
                destination_y_flipped,
                params.destination_start_x as i32 + params.width as i32,
                destination_y_flipped + params.height as i32,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );

            self.gl.delete_texture(temp_rgba_texture);
            self.gl.delete_framebuffer(temp_rgba_fbo);

            let temp_r16_texture = self.gl.create_texture().unwrap();
            let temp_r16_fbo = self.gl.create_framebuffer().unwrap();

            Self::bind_texture_to_framebuffer(
                &self.gl,
                glow::TEXTURE3,
                glow::R16UI,
                params.width as i32,
                params.height as i32,
                &temp_r16_texture,
                &temp_r16_fbo,
            );

            self.gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(self.fbo_read),
            );
            self.gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                Some(temp_r16_fbo),
            );

            self.gl.blit_framebuffer(
                params.source_start_x as i32,
                params.source_start_y as i32,
                params.source_start_x as i32 + params.width as i32,
                params.source_start_y as i32 + params.height as i32,
                0,
                0,
                params.width as i32,
                params.height as i32,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );

            self.gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                Some(temp_r16_fbo),
            );
            self.gl.bind_framebuffer(
                glow::DRAW_FRAMEBUFFER,
                Some(self.fbo_read),
            );

            self.gl.blit_framebuffer(
                0,
                0,
                params.width as i32,
                params.height as i32,
                params.destination_start_x as i32,
                params.destination_start_y as i32,
                params.destination_start_x as i32 + params.width as i32,
                params.destination_start_y as i32 + params.height as i32,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );

            self.gl.delete_texture(temp_r16_texture);
            self.gl.delete_framebuffer(temp_r16_fbo);
        }
    }

    pub fn get_vram_textures(&self) -> (Vec<u8>, Vec<u8>) {
        let mut rgba8_buf = vec![0u8; VRAM_WIDTH * VRAM_HEIGHT * 4];
        let mut rgba16_buf = vec![0u16; VRAM_WIDTH * VRAM_HEIGHT];

        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo_write));
            self.gl
                .read_pixels(
                    0,
                    0,
                    VRAM_WIDTH as i32,
                    VRAM_HEIGHT as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    PixelPackData::Slice(Some(&mut rgba8_buf)),
                );

            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo_read));
            {
                // let js_array =
                //     unsafe { Uint16Array::view_mut_raw(rgba16_buf.as_mut_ptr(), rgba16_buf.len()) };
                let slice: &mut [u8] = cast_slice_mut(&mut rgba16_buf);

                self.gl
                    .read_pixels(
                        0,
                        0,
                        VRAM_WIDTH as i32,
                        VRAM_HEIGHT as i32,
                        glow::RED_INTEGER,
                        glow::UNSIGNED_SHORT,
                        PixelPackData::Slice(Some(slice))
                    )
            }
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
