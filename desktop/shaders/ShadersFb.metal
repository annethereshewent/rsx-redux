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

// Helper function to read a byte from vram at a specific byte offset
uchar readByte(texture2d<ushort, access::read> vram, uint byteOffset, uint y) {
    uint halfwordX = byteOffset / 2;  // Which halfword contains this byte
    ushort halfword = vram.read(uint2(halfwordX, y)).r;

    if (byteOffset & 1) {
        // Odd byte - take high byte
        return uchar(halfword >> 8);
    } else {
        // Even byte - take low byte
        return uchar(halfword & 0xFF);
    }
}

// For reading display pixels in 24-bit mode
float4 readPixel24bit(
    texture2d<ushort, access::read> vram,
    uint displayStartX,
    uint displayPixelX,
    uint y
) {
    uint byteOffset = displayStartX * 2u + displayPixelX * 3u;

    uchar r = readByte(vram, byteOffset + 0u, y);
    uchar g = readByte(vram, byteOffset + 1u, y);
    uchar b = readByte(vram, byteOffset + 2u, y);

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
            vram,
            params.displayStartX,
            displayX,
            srcY
        );
    }
};