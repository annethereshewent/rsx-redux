#include <metal_stdlib>
using namespace metal;

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
float4 readPixel24bit(texture2d<ushort, access::read> vram, uint x, uint y) {
    uint byteOffset = x * 3;  // 3 bytes per pixel in 24-bit mode

    uchar r = readByte(vram, byteOffset, y);
    uchar g = readByte(vram, byteOffset + 1, y);
    uchar b = readByte(vram, byteOffset + 2, y);

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
    constant uint& displayDepth [[buffer(0)]]
) {
    if (displayDepth == 0) {
        constexpr sampler s(address::clamp_to_edge, filter::nearest);
        return tex.sample(s, in.uv);
    } else {
        uint x = uint(in.uv.x * 1024.0);
        uint y = uint(in.uv.y * 512.0);
        return readPixel24bit(vram, x, y);
    }
};