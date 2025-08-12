#include <metal_stdlib>
using namespace metal;

struct FragmentUniforms {
    bool hasTexture;
    uint textureMaskX;
    uint textureMaskY;
    uint textureOffsetX;
    uint textureOffsetY;
};

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 uv       [[attribute(1)]];
    float4 color    [[attribute(2)]];
    uint2 page [[attribute(3)]];
    uint depth [[attribute(4)]];
    uint2 clut [[attribute(5)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 uv [[center_no_perspective]];
    float4 color;
    uint2 page;
    uint depth;
    uint2 clut;
};

vertex VertexOut vertex_main(VertexIn in [[stage_in]]) {
    VertexOut out;
    out.position = float4(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.page = in.page;
    out.depth = in.depth;
    out.clut = in.clut;

    return out;
}
/*
let u_base = texpage.x_base * 64;
let v_base = texpage.y_base1 * 256;

let offset_u = 2 * u_base + u/2;
let offset_v = v_base + v;

let texture_address = 2 * (offset_u + 1024 * offset_v);

let mut texel_index = self.vram[texture_address as usize];

if u & 1 == 0 {
    texel_index &= 0xff
} else {
    texel_index = (texel_index >> 4) & 0xff;
}

let address = 2 * self.clut_x + 2048 * self.clut_y + texel_index as usize * 2;

let texel = unsafe { *(&self.vram[address] as *const u8 as *const u16) };

Self::convert_to_rgb888(texel)
*/
float4 getTexColor4bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = (uint(in.uv[0]) & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    uint v = (uint(in.uv[1]) & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = in.page[0] + u / 2;
    uint offsetV = in.page[1] + v;

    uint texelIndex = vram.read(uint2(offsetU, offsetV)).r;

    if ((u & 1) == 0) {
        texelIndex &= 0xf;
    } else {
        texelIndex = (texelIndex >> 4) & 0xf;
    }

    ushort texel = vram.read(uint2(texelIndex + in.clut[0], in.clut[1])).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    uint a = 255;

    if (texel == 0) {
        a = 0;
    }

    r = r << 3 | r >> 2;
    g = g << 3 | g >> 2;
    b = b << 3 | b >> 2;

    return float4(r, g, b, a) / 255.0;
}

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]],
                              texture2d<ushort, access::read> vram [[texture(0)]],
                              constant FragmentUniforms& uniforms [[buffer(1)]]
)
{
    if (uniforms.hasTexture) {
        float4 texColor = getTexColor4bpp(in, vram, uniforms);
        float4 finalColor = texColor * in.color;
        return texColor;
    } else {
        return float4(in.color);
    }
}
