use std::cmp;

use serde::{Deserialize, Serialize};

use crate::cpu::bus::gpu::{
    Color, DisplayDepth, GPU, Polygon, Semitransparency, Texpage, TexturePageColors, Vertex,
    deltas::Deltas,
};

#[derive(Serialize, Deserialize)]
struct Coordinate2d {
    x: i32,
    y: i32,
}

impl GPU {
    // TODO: add shaded line support. should be pretty straightforward
    pub fn rasterize_line(&mut self, polygon: &Polygon) {
        if self.debug_on {
            return;
        }

        let vertices = &polygon.vertices;
        let start_x = vertices[0].x;
        let start_y = vertices[0].y;

        let end_x = vertices[1].x;
        let end_y = vertices[1].y;

        let diff_x = end_x - start_x;
        let diff_y = end_y - start_y;

        if start_x < 0
            || end_x >= 1024
            || start_y < 0
            || end_y >= 512
            || !(0..512).contains(&start_y)
            || start_x >= 1024
        {
            return;
        }

        if diff_x != 0 {
            // line is either horizontal or diagonal
            let dydx = diff_y as f32 / diff_x as f32;

            let going_left = start_x > end_x;

            let (mut drdx, mut dgdx, mut dbdx) = if self.is_shaded {
                (
                    (vertices[1].color.r as f32 - vertices[0].color.r as f32) / diff_x as f32,
                    (vertices[1].color.g as f32 - vertices[0].color.g as f32) / diff_x as f32,
                    (vertices[1].color.b as f32 - vertices[0].color.b as f32) / diff_x as f32,
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            if going_left {
                drdx *= -1.0;
                dgdx *= -1.0;
                dbdx *= -1.0;
            }

            let mut r_fp = vertices[0].color.r as f32;
            let mut g_fp = vertices[0].color.g as f32;
            let mut b_fp = vertices[0].color.b as f32;

            let diff_x = diff_x.abs();

            let mut color = vertices[0].color;

            for x in 0..=diff_x {
                let curr_x = if going_left { start_x - x } else { x + start_x };
                let y_fp = x as f32 * dydx;

                if polygon.is_shaded {
                    color.r = r_fp as u8;
                    color.g = g_fp as u8;
                    color.b = b_fp as u8;

                    r_fp += drdx;
                    g_fp += dgdx;
                    b_fp += dbdx;
                }

                let curr_y = start_y + y_fp as i32;

                if curr_x < self.x1 as i32
                    || curr_x >= self.x2 as i32
                    || curr_y < self.y1 as i32
                    || curr_y >= self.y2 as i32
                {
                    continue;
                }

                let mut coordinates = Coordinate2d {
                    x: curr_x,
                    y: curr_y,
                };

                let mut output = color;

                if self.texpage.dither {
                    self.dither(&coordinates, &mut output);
                }

                self.render_pixel(polygon, &mut output, &coordinates);

                // if y_fp is not an integer, then there will be gaps in the diagonal line.
                // to counteract this, draw an extra pixel to make the line look "fuller"
                if y_fp.ceil() != y_fp {
                    coordinates.y += 1;
                    self.render_pixel(polygon, &mut output, &coordinates);
                }
            }
        } else {
            // line is vertical
            let (mut drdy, mut dgdy, mut dbdy) = if polygon.is_shaded {
                (
                    (vertices[1].color.r as f32 - vertices[0].color.r as f32) / diff_y as f32,
                    (vertices[1].color.g as f32 - vertices[0].color.g as f32) / diff_y as f32,
                    (vertices[1].color.b as f32 - vertices[0].color.b as f32) / diff_y as f32,
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            let diff_y = diff_y.abs();

            let going_up = start_y > end_y;

            if going_up {
                drdy *= -1.0;
                dgdy *= -1.0;
                dbdy *= -1.0;
            }

            let mut color = vertices[0].color;

            let mut r_fp = vertices[0].color.r as f32;
            let mut g_fp = vertices[0].color.g as f32;
            let mut b_fp = vertices[0].color.b as f32;

            if start_x < self.x1 as i32 || start_x >= self.x2 as i32 {
                return;
            }

            for y in 0..=diff_y {
                let curr_y = if going_up { start_y - y } else { y + start_y };

                if curr_y < self.y1 as i32 || curr_y >= self.y2 as i32 {
                    continue;
                }

                if polygon.is_shaded {
                    color.r = r_fp as u8;
                    color.g = g_fp as u8;
                    color.b = b_fp as u8;

                    r_fp += drdy;
                    g_fp += dgdy;
                    b_fp += dbdy;
                }

                let coordinates = Coordinate2d {
                    x: start_x,
                    y: curr_y,
                };

                let mut output = color;

                if self.texpage.dither {
                    self.dither(&coordinates, &mut output);
                }

                self.render_pixel(polygon, &mut output, &coordinates);
            }
        }
    }
    pub fn rasterize_triangle(&mut self, polygon: &mut Polygon) {
        polygon.vertices.sort_by(|a, b| a.y.cmp(&b.y));

        let cross_product = GPU::cross_product(&polygon.vertices);

        if cross_product == 0 {
            // malformed triangle
            return;
        }

        let mut min_x = cmp::min(
            polygon.vertices[0].x,
            polygon.vertices[1].x.min(polygon.vertices[2].x),
        );
        let mut max_x = cmp::max(
            polygon.vertices[0].x,
            polygon.vertices[1].x.max(polygon.vertices[2].x),
        );

        let mut min_y = cmp::min(
            polygon.vertices[0].y,
            polygon.vertices[1].y.min(polygon.vertices[2].y),
        );
        let mut max_y = cmp::max(
            polygon.vertices[0].y,
            polygon.vertices[1].y.max(polygon.vertices[2].y),
        );

        if (max_x >= 1024 && min_x >= 1024)
            || (max_x < 0 && min_x < 0)
            || (max_y >= 512 && min_y >= 512)
            || (max_y < 0 && min_y < 0)
            || max_x - min_x >= 1024
            || max_y - min_y >= 512
        {
            return;
        }

        min_x = cmp::max(min_x, self.x1 as i32);
        min_y = cmp::max(min_y, self.y1 as i32);

        max_x = cmp::min(max_x, self.x2 as i32);
        max_y = cmp::min(max_y, self.y2 as i32);

        let d = Deltas::get_deltas(polygon, cross_product as f32);

        // subtract from the first vertex's coordinates so that we start at the "origin" of the color so it's easier to compute the current color later
        let r_base = polygon.vertices[0].color.r as f32
            - d.drdx * polygon.vertices[0].x as f32
            - d.drdy * polygon.vertices[0].y as f32;
        let g_base = polygon.vertices[0].color.g as f32
            - d.dgdx * polygon.vertices[0].x as f32
            - d.dgdy * polygon.vertices[0].y as f32;
        let b_base = polygon.vertices[0].color.b as f32
            - d.dbdx * polygon.vertices[0].x as f32
            - d.dbdy * polygon.vertices[0].y as f32;

        let u_base = polygon.vertices[0].u as f32
            - d.dudx * polygon.vertices[0].x as f32
            - d.dudy * polygon.vertices[0].y as f32;
        let v_base = polygon.vertices[0].v as f32
            - d.dvdx * polygon.vertices[0].x as f32
            - d.dvdy * polygon.vertices[0].y as f32;

        let p01_slope = if polygon.vertices[0].y != polygon.vertices[1].y {
            Some(
                (polygon.vertices[1].x - polygon.vertices[0].x) as f32
                    / (polygon.vertices[1].y - polygon.vertices[0].y) as f32,
            )
        } else {
            None
        };

        let p12_slope = if polygon.vertices[1].y != polygon.vertices[2].y {
            Some(
                (polygon.vertices[2].x - polygon.vertices[1].x) as f32
                    / (polygon.vertices[2].y - polygon.vertices[1].y) as f32,
            )
        } else {
            None
        };

        let p02_slope = (polygon.vertices[2].x - polygon.vertices[0].x) as f32
            / (polygon.vertices[2].y - polygon.vertices[0].y) as f32;

        let p02_is_left = cross_product > 0;

        let mut curr_color = polygon.vertices[0].color;

        let mut curr_point = Coordinate2d { x: min_x, y: min_y };

        while curr_point.y < max_y {
            curr_point.x = min_x;
            while curr_point.x < max_x {
                let (boundary1, boundary2) =
                    polygon.get_boundaries(p01_slope, p12_slope, p02_slope, &curr_point);

                let (curr_min_x, curr_max_x) = if p02_is_left {
                    (boundary2, boundary1)
                } else {
                    (boundary1, boundary2)
                };

                if curr_point.x >= curr_min_x && curr_point.x < curr_max_x {
                    // render the pixel!
                    if polygon.is_shaded {
                        Self::interpolate_color(
                            &mut curr_color,
                            &curr_point,
                            r_base,
                            g_base,
                            b_base,
                            &d,
                        );

                        if self.texpage.dither {
                            self.dither(&curr_point, &mut curr_color);
                        }
                    }
                    let mut output = curr_color;

                    if let Some(texpage) = polygon.texpage {
                        let uv = polygon.interpolate_texture_coordinates(
                            &curr_point,
                            u_base,
                            v_base,
                            &d,
                        );

                        let masked_uv = self.mask_texture_coordinates(uv);

                        if let Some(mut texture) = self.get_texture(polygon, texpage, masked_uv) {
                            if polygon.modulate {
                                Self::modulate_texture(&output, &mut texture);
                                if texpage.dither {
                                    self.dither(&curr_point, &mut texture);
                                }
                            }
                            output = texture;
                        } else {
                            curr_point.x += 1;
                            continue;
                        }
                    }

                    self.render_pixel(polygon, &mut output, &curr_point);
                }
                curr_point.x += 1;
            }
            curr_point.y += 1;
        }
    }

    fn modulate_texture(&curr_color: &Color, texture: &mut Color) {
        texture.r = cmp::min(255, ((curr_color.r as u32) * (texture.r as u32)) >> 7) as u8;
        texture.g = cmp::min(255, ((curr_color.g as u32) * (texture.g as u32)) >> 7) as u8;
        texture.b = cmp::min(255, ((curr_color.b as u32) * (texture.b as u32)) >> 7) as u8;
    }

    fn render_pixel(&mut self, polygon: &Polygon, output: &mut Color, curr_point: &Coordinate2d) {
        let vram_address =
            Self::get_vram_address(curr_point.x as u32 & 0x3ff, curr_point.y as u32 & 0x1ff);

        let previous_color = Color::translate15bit_to_24(unsafe {
            *(&self.vram[Self::get_vram_address(
                (curr_point.x as u32) & 0x3ff,
                (curr_point.y as u32) & 0x1ff,
            )] as *const u8 as *const u16)
        });

        if previous_color.a != 0 && self.preserve_masked_pixels {
            return;
        }

        let texpage = if let Some(texpage) = polygon.texpage {
            texpage
        } else {
            self.texpage
        };

        if (!polygon.textured || output.a == 1) && polygon.semitransparent {
            match texpage.semi_transparency {
                Semitransparency::Half => Self::semitransparent_half(output, &previous_color),
                Semitransparency::Add => Self::semitransparent_add(output, &previous_color),
                Semitransparency::Subtract => {
                    Self::semitransparent_subtract(output, &previous_color)
                }
                Semitransparency::Quarter => Self::semitransparent_quarter(output, &previous_color),
            }
        }

        let r = output.r >> 3;
        let g = output.g >> 3;
        let b = output.b >> 3;

        let mut output = r as u16 | (g as u16) << 5 | (b as u16) << 10 | (output.a as u16) << 15;

        if self.force_mask_bit {
            output |= 1 << 15;
        }

        unsafe { *(&mut self.vram[vram_address] as *mut u8 as *mut u16) = output };
    }

    fn semitransparent_half(output: &mut Color, previous_color: &Color) {
        output.r = 255.min((output.r as u32 + previous_color.r as u32) / 2) as u8;
        output.g = 255.min((output.g as u32 + previous_color.g as u32) / 2) as u8;
        output.b = 255.min((output.b as u32 + previous_color.b as u32) / 2) as u8;
    }

    fn semitransparent_add(output: &mut Color, previous_color: &Color) {
        output.r = 255.min(output.r as u32 + previous_color.r as u32) as u8;
        output.g = 255.min(output.g as u32 + previous_color.g as u32) as u8;
        output.b = 255.min(output.b as u32 + previous_color.b as u32) as u8;
    }

    fn semitransparent_subtract(output: &mut Color, previous_color: &Color) {
        output.r = (previous_color.r as i32 - output.r as i32).clamp(0, 255) as u8;
        output.g = (previous_color.g as i32 - output.g as i32).clamp(0, 255) as u8;
        output.b = (previous_color.b as i32 - output.b as i32).clamp(0, 255) as u8;
    }

    fn semitransparent_quarter(output: &mut Color, previous_color: &Color) {
        output.r = 255.min(output.r as u32 / 4 + previous_color.r as u32) as u8;
        output.g = 255.min(output.g as u32 / 4 + previous_color.g as u32) as u8;
        output.b = 255.min(output.b as u32 / 4 + previous_color.b as u32) as u8;
    }

    fn interpolate_color(
        color: &mut Color,
        curr_point: &Coordinate2d,
        r_base: f32,
        g_base: f32,
        b_base: f32,
        d: &Deltas,
    ) {
        color.r = (d.drdx * curr_point.x as f32 + d.drdy * curr_point.y as f32 + r_base) as u8;
        color.g = (d.dgdx * curr_point.x as f32 + d.dgdy * curr_point.y as f32 + g_base) as u8;
        color.b = (d.dbdx * curr_point.x as f32 + d.dbdy * curr_point.y as f32 + b_base) as u8;
    }

    fn get_texture(&self, polygon: &Polygon, texpage: Texpage, uv: (u8, u8)) -> Option<Color> {
        match texpage.texture_page_colors {
            TexturePageColors::Bit4 => self.read_4bit_clut(polygon, texpage, uv),
            TexturePageColors::Bit8 => self.read_8bit_clut(polygon, texpage, uv),
            TexturePageColors::Bit15 => self.read_15bit_clut(texpage, uv),
        }
    }

    fn dither(&mut self, position: &Coordinate2d, pixel: &mut Color) {
        let x = (position.x & 3) as usize;
        let y = (position.y & 3) as usize;

        pixel.r = self.dither_table[x][y][pixel.r as usize];
        pixel.g = self.dither_table[x][y][pixel.g as usize];
        pixel.b = self.dither_table[x][y][pixel.b as usize];
    }

    fn read_4bit_clut(&self, polygon: &Polygon, texpage: Texpage, uv: (u8, u8)) -> Option<Color> {
        let (tex_x_base, tex_y_base) = (texpage.x_base * 64, texpage.y_base1 * 16);

        let offset_u = (2 * tex_x_base + uv.0 as u32 / 2) & 2047;
        let offset_v = (tex_y_base + uv.1 as u32) & 511;

        let clut_index_address = offset_u + offset_v * 2048;

        let byte = self.vram[clut_index_address as usize];

        let texel_index = if uv.0 & 1 == 1 { byte >> 4 } else { byte & 0xf };

        let texture_address = 2 * (texel_index as u32 + polygon.clut.0 + polygon.clut.1 * 1024);

        let texture = unsafe { *(&self.vram[texture_address as usize] as *const u8 as *const u16) };

        if texture == 0 {
            None
        } else {
            Some(Color::translate15bit_to_24(texture))
        }
    }

    fn read_8bit_clut(&self, polygon: &Polygon, texpage: Texpage, uv: (u8, u8)) -> Option<Color> {
        let (tex_x_base, tex_y_base) = (texpage.x_base * 64, texpage.y_base1 * 16);

        let offset_u = (2 * tex_x_base + uv.0 as u32) & 2047;
        let offset_v = (tex_y_base + uv.1 as u32) & 511;

        let clut_index_address = offset_u + offset_v * 2048;

        let texel_index = self.vram[clut_index_address as usize];

        let texture_address = 2 * (texel_index as u32 + polygon.clut.0 + polygon.clut.1 * 1024);

        let texture = unsafe { *(&self.vram[texture_address as usize] as *const u8 as *const u16) };

        if texture == 0 {
            None
        } else {
            Some(Color::translate15bit_to_24(texture))
        }
    }

    fn read_15bit_clut(&self, texpage: Texpage, uv: (u8, u8)) -> Option<Color> {
        let (tex_x_base, tex_y_base) = (texpage.x_base * 64, texpage.y_base1 * 16);

        let offset_u = (tex_x_base + uv.0 as u32) & 2047;
        let offset_v = (tex_y_base + uv.1 as u32) & 511;

        let texture_address = 2 * (offset_u + 1024 * offset_v);

        let texture = unsafe { *(&self.vram[texture_address as usize] as *const u8 as *const u16) };

        if texture == 0 {
            None
        } else {
            Some(Color::translate15bit_to_24(texture))
        }
    }

    fn mask_texture_coordinates(&self, uv: (u8, u8)) -> (u8, u8) {
        let mask_x = self.texture_window_mask_x;
        let mask_y = self.texture_window_mask_y;

        let offset_x = self.texture_window_offset_x;
        let offset_y = self.texture_window_offset_y;

        let masked_u = (uv.0 as u32 & !mask_x) | (offset_x & mask_x);
        let masked_v = (uv.1 as u32 & !mask_y) | (offset_y & mask_y);

        (masked_u as u8, masked_v as u8)
    }

    pub fn update_framebuffer(&mut self) {
        let (width, height) = self.get_dimensions();
        let mut i = 0;

        let display_start_x = self.display_start_x;
        let display_start_y = self.display_start_y;

        for y in display_start_y..display_start_y + height {
            for x in 0..width {
                match self.display_depth {
                    DisplayDepth::Bit15 => {
                        let curr_x = x + display_start_x;
                        let vram_address = GPU::get_vram_address(curr_x & 0x3ff, y & 0x1ff);

                        let halfword =
                            unsafe { *(&self.vram[vram_address] as *const u8 as *const u16) };

                        let color = Color::translate15bit_to_24(halfword);

                        self.picture[i] = color.r;
                        self.picture[i + 1] = color.g;
                        self.picture[i + 2] = color.b;
                    }
                    DisplayDepth::Bit24 => {
                        let byte_offset = display_start_x as usize * 2 + x as usize * 3;

                        let row_base = y as usize * 1024 * 2;
                        let vram_address = row_base + byte_offset;

                        self.picture[i] = self.vram[vram_address];
                        self.picture[i + 1] = self.vram[vram_address + 1];
                        self.picture[i + 2] = self.vram[vram_address + 2];
                    }
                }

                i += 3;
            }
        }
    }
}

impl Polygon {
    /// Gets the left and right boundaries of the triangle to where to render pixels to. Considers the 3 possible slopes of a triangle and determines the left boundary
    /// and the right boundary based on them.
    fn get_boundaries(
        &self,
        p01_slope: Option<f32>,
        p12_slope: Option<f32>,
        p02_slope: f32,
        curr_point: &Coordinate2d,
    ) -> (i32, i32) {
        // Three possible cases to consider:
        // p01 slope is horizontal
        // p12 slope is horizontal
        // neither are horizontal

        let boundary1 = if p01_slope.is_none() {
            if let Some(slope) = p12_slope {
                Self::get_boundary_from_slope(&self.vertices[1], slope, curr_point)
            } else {
                println!("shouldn't happen: p01_slope and p12_slope are both None");
                0
            }
        } else if p12_slope.is_none() {
            if let Some(slope) = p01_slope {
                Self::get_boundary_from_slope(&self.vertices[0], slope, curr_point)
            } else {
                println!("shouldn't happen: p01_slope and p12_slope are both None");
                0
            }
        } else {
            // determine what slope to use based on whether the current point is less than the y coordinate of the triangle vertices (vertex 0 vs vertex 1)
            if curr_point.y <= self.vertices[1].y {
                // Use p01_slope
                if let Some(slope) = p01_slope {
                    Self::get_boundary_from_slope(&self.vertices[0], slope, curr_point)
                } else {
                    println!("shouldn't happen: p01_slope is None but should be Some");
                    0
                }
            } else if let Some(slope) = p12_slope {
                Self::get_boundary_from_slope(&self.vertices[1], slope, curr_point)
            } else {
                println!("shouldn't happen: p12_slope is None but should be Some");
                0
            }
        };

        // p02 slope is never horizontal, as vertices are sorted by y coordinate, and thus it is always either vertical or diagonal. so we can always use it to calculate the
        // 2nd boundary
        let boundary2 = Self::get_boundary_from_slope(&self.vertices[0], p02_slope, curr_point);

        (boundary1, boundary2)
    }

    fn get_boundary_from_slope(base_vertex: &Vertex, slope: f32, curr_point: &Coordinate2d) -> i32 {
        let rel_y = (curr_point.y - base_vertex.y) as f32;

        (slope * rel_y) as i32 + base_vertex.x
    }

    fn interpolate_texture_coordinates(
        &self,
        curr_point: &Coordinate2d,
        u_base: f32,
        v_base: f32,
        d: &Deltas,
    ) -> (u8, u8) {
        let u = (curr_point.x as f32 * d.dudx + curr_point.y as f32 * d.dudy + u_base) as u8;
        let v = (curr_point.x as f32 * d.dvdx + curr_point.y as f32 * d.dvdy + v_base) as u8;

        (u, v)
    }
}
