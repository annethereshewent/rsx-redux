#include <metal_stdlib>
using namespace metal;

struct FragmentUniforms {
    bool hasTexture;
    bool isShaded;
};

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 uv       [[attribute(1)]];
    float4 color    [[attribute(2)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float4 color;
};

vertex VertexOut vertex_main(VertexIn in [[stage_in]]) {
    VertexOut out;
    out.position = float4(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]])
{
    // if (uniforms.hasTexture) {
    //     float4 texColor = tex.sample(textureSampler, in.uv);
    //     float4 finalColor = texColor * in.color;

    //     return finalColor;
    // } else {
    //     return float4(in.color);
    // }

    return float4(in.color);
}
