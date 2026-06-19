#version 300 es
precision highp float;
precision highp usampler2D;

uniform sampler2D vramWrite;

in vec2 vUv;
out uint outHalfword;

void main() {
    vec2 flippedUv = vec2(vUv.x, 1.0 - vUv.y);
    vec4 rgba = texture(vramWrite, flippedUv);

    uint r = uint(rgba.r * 255.0 + 0.5) >> 3u;
    uint g = uint(rgba.g * 255.0 + 0.5) >> 3u;
    uint b = uint(rgba.b * 255.0 + 0.5) >> 3u;
    uint a = rgba.a != 0.0 ? 1u : 0u; // bit 15

    outHalfword = (a << 15u) | (b << 10u) | (g << 5u) | r;
}
