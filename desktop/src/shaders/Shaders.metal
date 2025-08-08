struct VertexIn {
    float3 position [[attribute(0)]];
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
    out.position = float4(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

// Fragment
fragment float4 fragment_main(VertexOut in [[stage_in]],
                              texture2d<float> tex [[texture(0)]],
                              constant FragmentUniforms& uniforms [[buffer(1)]],
                              sampler textureSampler [[sampler(0)]])
{
    if (uniforms.hasTexture) {
        if (uniforms.clampS) {
            in.uv.x = clamp(in.uv.x, 0.0, 1.0);
        }
        if (uniforms.clampT) {
            in.uv.y = clamp(in.uv.y, 0.0, 1.0);
        }

        float4 texColor = tex.sample(textureSampler, in.uv);
        float4 finalColor = texColor * in.color;

        return finalColor;
    } else {
        return float4(in.color);
    }
}
