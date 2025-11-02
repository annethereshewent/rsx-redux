#include <metal_stdlib>
using namespace metal;

struct FragmentUniforms {
    bool hasTexture;
    bool semitransparent;
    uint textureMaskX;
    uint textureMaskY;
    uint textureOffsetX;
    uint textureOffsetY;
    int depth;
    uint transparentMode;
    uint pass;
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

// TODO: actually implement CLUT
float4 getTexColor16bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = (uint(in.uv[0]) & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    uint v = (uint(in.uv[1]) & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = in.page[0] + u;
    uint offsetV = in.page[1] + v;

    ushort texel = vram.read(uint2(offsetU, offsetV)).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    float a = 31.0;

    if (texel == 0) {
        a = -31.0;
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
    float a = float((texel >> 15) & 1) * 31.0;

    if (texel == 0) {
        a = -31.0;
    }

    return float4(r, g, b, a) / 31.0;
}

float4 getTexColor8bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = (uint(in.uv[0]) & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    uint v = (uint(in.uv[1]) & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = in.page[0] + u / 2;
    uint offsetV = in.page[1] + v;

    uint halfWord = vram.read(uint2(offsetU, offsetV)).r;

    uint texelIndex = (u & 1) == 0 ? halfWord & 0xff : halfWord >> 8;

    ushort texel = vram.read(uint2(texelIndex + in.clut[0], in.clut[1])).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    float a = float((texel >> 15) & 1) * 31.0;

    if (texel == 0) {
        a = -31.0;
    }

    return float4(r, g, b, a) / 31.0;
}

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]],
                              texture2d<ushort, access::read> vram [[texture(0)]],
                              constant FragmentUniforms& uniforms [[buffer(1)]]
)
{
    float4 finalColor;
    if (uniforms.hasTexture) {
        float4 texColor;
        switch (uniforms.depth) {
            case 0:
                texColor = getTexColor4bpp(in, vram, uniforms);
                break;
            case 1:
                texColor = getTexColor8bpp(in, vram, uniforms);
                break;
            case 2:
                texColor = getTexColor16bpp(in, vram, uniforms);
                break;
        }

        if (texColor[3] < 0.0) {
            discard_fragment();
        }

        finalColor = texColor;
    } else {
        finalColor = float4(in.color);
    }

    float alpha = 1.0;

    bool isST = uniforms.hasTexture ? finalColor[3] > 0.5 : uniforms.semitransparent;

    if (uniforms.semitransparent) {
        float2 screen = float2(
            (in.position.x * 0.5 + 0.5) * 1024.0,
            (1.0 - (in.position.y * 0.5 + 0.5)) * 512.0
        );
        ushort2 coord = ushort2(clamp(screen, float2(0.0), float2(1023.0, 511.0)));
        ushort pixel = vram.read(coord).r;

        uint r = pixel & 0x1f;
        uint g = (pixel >> 5) & 0x1f;
        uint b = (pixel >> 10) & 0x1f;

        float4 old = float4(r, g, b, 31.0) / 31.0;


        switch (uniforms.transparentMode) {
            case 0:
                finalColor = max((old + finalColor) / 2, 1.0);
                break;
            case 1:
                finalColor = max(old + finalColor, 1.0);
                break;
            case 2:
                finalColor = min(old - finalColor, 0.0);
                break;
            case 3:
                finalColor = max(old + (finalColor / 4), 1.0);
                break;
        }
    }

    finalColor[3] = alpha;

    return finalColor;
}

kernel void rgba8_to_rgb5551(texture2d<float, access::sample> src [[texture(0)]],
                             texture2d<ushort, access::write> dst [[texture(1)]],
                             constant uint2 &dstOrigin [[buffer(0)]],
                             uint2 gid [[thread_position_in_grid]]) {
    float3 c = src.read(gid).rgb;
    ushort r = ushort(c.r * 31.0);
    ushort g = ushort(c.g * 31.0);
    ushort b = ushort(c.b * 31.0);
    ushort a = (r | g | b) ? 1 : 0;
    ushort packed = r | (g << 5) | (b << 10) | (a << 15);
    dst.write(packed, dstOrigin + gid);
}