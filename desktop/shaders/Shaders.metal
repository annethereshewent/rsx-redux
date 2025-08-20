#include <metal_stdlib>
using namespace metal;

struct FragmentUniforms {
    bool hasTexture;
    uint textureMaskX;
    uint textureMaskY;
    uint textureOffsetX;
    uint textureOffsetY;
    int depth;
};

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 uv       [[attribute(1)]];
    float4 color    [[attribute(2)]];
    uint2 page [[attribute(3)]];
    uint2 clut [[attribute(4)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 uv [[center_no_perspective]];
    float4 color;
    uint2 page;
    uint2 clut;
};

vertex VertexOut vertex_main(VertexIn in [[stage_in]]) {
    VertexOut out;
    out.position = float4(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.page = in.page;
    out.clut = in.clut;

    return out;
}

/*
  fn read_texture(&mut self, uv: Coordinates2d) -> Option<RgbColor> {
    let tex_x_base = (self.stat.texture_x_base as i32) * 64;
    let tex_y_base = (self.stat.texture_y_base1 as i32) * 16;

    let offset_x = tex_x_base + uv.x;
    let offset_y = tex_y_base + uv.y;

    let texture_address = 2 * (offset_x + offset_y * 1024) as usize;

    // for this case, each cache entry is 8 * 32 cache lines, and each cache entry is 4 16bpp pixels wide
    let entry = (((uv.y * 8) + ((uv.x / 4 ) & 0x7)) & 0xff) as usize;
    let block = ((offset_x / 32) + (offset_y / 32) * 8) as isize;

    let cache_entry = &mut self.texture_cache[entry];

    if cache_entry.tag != block {
      for i in 0..8 {
        cache_entry.data[i] = self.vram[(texture_address & !0x7) + i];
      }
    }

    let index = ((uv.x * 2) & 0x7) as usize;

    let texture = (cache_entry.data[index] as u16) | (cache_entry.data[index + 1] as u16) << 8;

    if texture != 0 {
      Some(GPU::translate_15bit_to_24(texture))
    } else {
      None
    }
  }
*/
float4 getTexColor16bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = (uint(in.uv[0]) & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    uint v = (uint(in.uv[1]) & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = in.page[0] + u;
    uint offsetV = in.page[1] + v;

    ushort texel = vram.read(uint2(offsetU, offsetV)).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    uint a = 31;

    if (texel == 0) {
        a = 0;
    }

    return float4(r, g, b, a) / 31.0;
}

float4 getTexColor4bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = (uint(in.uv[0]) & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    uint v = (uint(in.uv[1]) & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = in.page[0] + u / 4;
    uint offsetV = in.page[1] + v;

    uint halfWord = vram.read(uint2(offsetU, offsetV)).r;

    uint texelIndex = ((u >> 1) & 1) == 0 ? halfWord & 0xff : (halfWord >> 8) & 0xff;

    if ((u & 1) == 0) {
        texelIndex &= 0xf;
    } else {
        texelIndex = (texelIndex >> 4) & 0xf;
    }

    ushort texel = vram.read(uint2(texelIndex + in.clut[0], in.clut[1])).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    // normally would be 255, but it's easier just to divide everything by 31
    uint a = 31;

    if (texel == 0) {
        a = 0;
    }

    return float4(r, g, b, a) / 31.0;
}

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]],
                              texture2d<ushort, access::read> vram [[texture(0)]],
                              constant FragmentUniforms& uniforms [[buffer(1)]]
)
{
    if (uniforms.hasTexture) {
        float4 texColor;
        switch (uniforms.depth) {
            case 0:
                texColor = getTexColor4bpp(in, vram, uniforms);
                break;
            case 2:
                texColor = getTexColor16bpp(in, vram, uniforms);
                break;
        }

        float4 finalColor = texColor * in.color;
        return texColor;
    } else {
        return float4(in.color);
    }
}
