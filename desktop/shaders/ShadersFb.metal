#include <metal_stdlib>
using namespace metal;

struct FbParams {
    uint displayStartX;
    uint displayStartY;
    uint displayWidth;
    uint displayHeight;
    uint displayDepth;
};

struct VertexFbIn {
    float2 position [[attribute(0)]];
    float2 uv       [[attribute(1)]];
};

struct VertexFbOut {
    float4 position [[position]];
    float2 uv;
};

uchar readByteFromRGBA8(texture2d<float> tex, uint byteOffset, uint y) {
    uint pixelX = byteOffset / 2;
    float4 rgba = tex.read(uint2(pixelX, y));

    uint r5 = uint(rgba.r * 255.0 + 0.5) >> 3;
    uint g5 = uint(rgba.g * 255.0 + 0.5) >> 3;
    uint b5 = uint(rgba.b * 255.0 + 0.5) >> 3;
    uint a1 = rgba.a > 0.5 ? 1u : 0u;

    uint halfword = r5 | (g5 << 5) | (b5 << 10) | (a1 << 15);

    if (byteOffset & 1) {
        return uchar(halfword >> 8);
    } else {
        return uchar(halfword & 0xFF);
    }
}

float4 readPixel24bit(
    texture2d<float> tex,
    uint displayStartX,
    uint displayPixelX,
    uint y
) {
    uint byteOffset = displayStartX * 2u + displayPixelX * 3u;

    uchar r = readByteFromRGBA8(tex, byteOffset + 0u, y);
    uchar g = readByteFromRGBA8(tex, byteOffset + 1u, y);
    uchar b = readByteFromRGBA8(tex, byteOffset + 2u, y);

    return float4(float(r) / 255.0, float(g) / 255.0, float(b) / 255.0, 1.0);
}

vertex VertexFbOut vertex_fb(uint vid [[vertex_id]],
                               const device VertexFbIn* verts [[buffer(0)]]) {
    VertexFbOut out;
    out.position = float4(verts[vid].position, 0.0, 1.0);
    out.uv = verts[vid].uv;
    return out;
};

fragment float4 fragment_fb(
    VertexFbOut in [[stage_in]],
    texture2d<float> tex [[texture(0)]],
    texture2d<ushort, access::read> vram [[texture(1)]],
    constant FbParams& params [[buffer(0)]]
) {
    uint srcX = params.displayStartX + uint(in.uv.x * float(params.displayWidth));
    uint srcY = params.displayStartY + uint(in.uv.y * float(params.displayHeight));

    srcX = min(srcX, 1023u);
    srcY = min(srcY, 511u);

    if (params.displayDepth == 0) {
        return tex.read(uint2(srcX, srcY));
    } else {
        uint displayX = min(
            uint(in.uv.x * float(params.displayWidth)),
            params.displayWidth - 1u
        );

        return readPixel24bit(
            tex,
            params.displayStartX,
            displayX,
            srcY
        );
    }
};