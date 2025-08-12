#include <metal_stdlib>
using namespace metal;

struct FragmentUniforms {
    bool hasTexture;
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
    float2 uv;
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

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]],
                              texture2d<ushort, access::read> vram [[texture(0)]],
                              constant FragmentUniforms& uniforms [[buffer(1)]]
)
{
    if (uniforms.hasTexture) {
        // float4 texColor = tex.sample(textureSampler, in.uv);
        // float4 finalColor = texColor * in.color;
        return float4(in.color);
    } else {
        return float4(in.color);
    }
}
