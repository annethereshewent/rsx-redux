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
        switch (uniforms.transparentMode) {
            case 0: alpha = isST ? 0.5 : 1.0; break;
            case 1: alpha = isST ? 1.0 : 0.0; break;
            case 2:
                if (!uniforms.hasTexture) {
                    alpha = 1.0;
                } else if (uniforms.pass == 1) {
                    if (isST) {
                        discard_fragment();
                    }
                } else if (!isST) {
                    discard_fragment();
                }
                break;
            case 3:
                if (!uniforms.hasTexture) {
                    alpha = 0.25;
                } else if (uniforms.pass == 1) {
                    if (isST) {
                        discard_fragment();
                    }
                } else {
                    if (!isST) {
                        discard_fragment();
                    }
                    alpha = 0.25;
                }
                break;
        }
    }

    finalColor[3] = alpha;

    return finalColor;
}
