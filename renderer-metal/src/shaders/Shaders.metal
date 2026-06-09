#include <metal_stdlib>
using namespace metal;

struct FragmentUniforms {
    bool hasTexture;
    bool semitransparent;
    bool modulate;
    uint textureMaskX;
    uint textureMaskY;
    uint textureOffsetX;
    uint textureOffsetY;
    int depth;
    uint transparentMode;
    uint pass;
    uint2 page;
    uint2 clut;
    bool forceMaskBit;
    bool preserveMaskedPixels;
};

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 uv       [[attribute(1)]];
    float4 color    [[attribute(2)]];
    float2 orig [[attribute(3)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 uv [[center_no_perspective]];
    float4 color;
    float2 orig;
};

vertex VertexOut vertex_main(VertexIn in [[stage_in]]) {
    VertexOut out;
    out.position = float4(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.orig = in.orig;

    return out;
}

// old 16bpp function, uses vram_read
float4 getTexColor16bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = uint(in.uv.x) & 0xffu;
    uint v = uint(in.uv.y) & 0xffu;

    u = (u & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    v = (v & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = uniforms.page[0] + u;
    uint offsetV = uniforms.page[1] + v;

    ushort texel = vram.read(uint2(offsetU, offsetV)).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    float a = float((texel >> 15) & 1) * 31.0;

    if (texel == 0) {
        discard_fragment();
    }

    return float4(r, g, b, a) / 31.0;
}

// new 16bpp function with vram_blend
float4 getTexColor16bpp(VertexOut in, texture2d<float, access::read> vram, FragmentUniforms uniforms) {
    uint u = uint(in.uv.x) & 0xffu;
    uint v = uint(in.uv.y) & 0xffu;

    u = (u & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    v = (v & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = uniforms.page[0] + u;
    uint offsetV = uniforms.page[1] + v;

    float4 color = vram.read(uint2(offsetU, offsetV));

    if (color.r == 0 && color.g == 0 && color.b == 0 && color.a == 0) {
        discard_fragment();
    }

    return color;
}

float4 getTexColor4bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = uint(in.uv.x) & 0xffu;
    uint v = uint(in.uv.y) & 0xffu;

    u = (u & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    v = (v & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = uniforms.page[0] + u / 4;
    uint offsetV = uniforms.page[1] + v;

    uint halfWord = vram.read(uint2(offsetU, offsetV)).r;

    uint texelIndex = ((u >> 1) & 1) == 0 ? halfWord & 0xff : (halfWord >> 8) & 0xff;

    if ((u & 1) == 0) {
        texelIndex &= 0xf;
    } else {
        texelIndex = (texelIndex >> 4) & 0xf;
    }

    ushort texel = vram.read(uint2(texelIndex + uniforms.clut.x, uniforms.clut.y)).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    float a = float((texel >> 15) & 1) * 31.0;

    if (texel == 0) {
        discard_fragment();
    }

    return float4(r, g, b, a) / 31.0;
}

float4 getTexColor8bpp(VertexOut in, texture2d<ushort, access::read> vram, FragmentUniforms uniforms) {
    uint u = uint(in.uv.x) & 0xffu;
    uint v = uint(in.uv.y) & 0xffu;

    u = (u & ~uniforms.textureMaskX) | (uniforms.textureOffsetX & uniforms.textureMaskX);
    v = (v & ~uniforms.textureMaskY) | (uniforms.textureOffsetY & uniforms.textureMaskY);

    uint offsetU = uniforms.page[0] + u / 2;
    uint offsetV = uniforms.page[1] + v;

    uint halfWord = vram.read(uint2(offsetU, offsetV)).r;

    uint texelIndex = (u & 1) == 0 ? halfWord & 0xff : halfWord >> 8;

    ushort texel = vram.read(uint2(texelIndex + uniforms.clut.x, uniforms.clut.y)).r;

    uint r = texel & 0x1f;
    uint g = (texel >> 5) & 0x1f;
    uint b = (texel >> 10) & 0x1f;

    float a = float((texel >> 15) & 1) * 31.0;

    if (texel == 0) {
        discard_fragment();
    }

    return float4(r, g, b, a) / 31.0;
}

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]],
                              float4 currentColor [[color(0)]],
                              texture2d<ushort, access::read> vram [[texture(0)]],
                              texture2d<float, access::read> vramSample [[texture(1)]],
                              constant FragmentUniforms& uniforms [[buffer(1)]]
)
{
    float4 finalColor;
    float texAlpha = 0;

    if (currentColor.a != 0 && uniforms.preserveMaskedPixels) {
        discard_fragment();
    }

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
                // Note: getTexColor16bpp uses the vram sample texture since
                // it's the same format as vram_write, which makes it convenient
                // to just sample from and fixes a lot of issues with games
                texColor = getTexColor16bpp(in, vramSample, uniforms);

                break;
        }

        texAlpha = texColor[3];

        if (uniforms.modulate) {
            uint4 texColorUint = uint4(texColor * 255.0);
            uint4 vertColorUint = uint4(in.color * 255.0);

            texColorUint = min((texColorUint * vertColorUint) >> 7, 0xff);

            texColor = float4(texColorUint) / 255.0;
        }

        finalColor = texColor;
    } else {
        finalColor = float4(in.color);
    }

    if (uniforms.semitransparent && (!uniforms.hasTexture || texAlpha == 1)) {
        float4 old = currentColor;

        switch (uniforms.transparentMode) {
            case 0:
                finalColor = min((old + finalColor) / 2, 1.0);
                break;
            case 1:
                finalColor = min(old + finalColor, 1.0);
                break;
            case 2:
                finalColor = max(old - finalColor, 0.0);
                break;
            case 3:
                finalColor = min(old + (finalColor / 4), 1.0);
                break;
        }
    }

    if (uniforms.forceMaskBit) {
        finalColor.a = 1.0;
    } else if (uniforms.hasTexture) {
        finalColor.a = texAlpha;
    } else {
        finalColor.a = 0.0;
    }

    return finalColor;
}

kernel void rgba8_to_rgb5551(texture2d<float, access::sample> src [[texture(0)]],
                             texture2d<ushort, access::write> dst [[texture(1)]],
                             constant uint2 &dstOrigin [[buffer(0)]],
                             uint2 gid [[thread_position_in_grid]]) {
    float4 c = src.read(gid + dstOrigin);
    ushort r = ushort(c.r * 255.0 + 0.5) >> 3;
    ushort g = ushort(c.g * 255.0 + 0.5) >> 3;
    ushort b = ushort(c.b * 255.0 + 0.5) >> 3;
    ushort a = c.a > 0.5 ? 1 : 0;
    ushort packed = r | (g << 5) | (b << 10) | (a << 15);
    dst.write(packed, dstOrigin + gid);
}