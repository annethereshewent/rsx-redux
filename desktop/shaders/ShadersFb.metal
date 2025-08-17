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

vertex VertexFbOut vertex_fb(uint vid [[vertex_id]],
                               const device VertexFbIn* verts [[buffer(0)]]) {
    VertexFbOut out;
    out.position = float4(verts[vid].position, 0.0, 1.0);
    out.uv = verts[vid].uv;
    return out;
};

fragment float4 fragment_fb(VertexFbOut in [[stage_in]],
                               texture2d<float> tex [[texture(0)]]) {
    constexpr sampler s(address::clamp_to_edge, filter::nearest);
    return tex.sample(s, in.uv);
};