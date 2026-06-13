#version 300 es
precision highp float;

uniform sampler2D vramWrite;
uniform uint displayDepth;
uniform uvec2 displayStart;
uniform uvec2 displaySize;

in vec2 vUv;
out vec4 outColor;

void main() {
    vec2 vramUv = vec2(0u, 0u);
    vramUv.x = (float(displayStart.x) + vUv.x * float(displaySize.x)) / 1024.0;
    vramUv.y = (float(512u - displayStart.y - displaySize.y) + vUv.y * float(displaySize.y)) / 512.0;

    if (displayDepth == 0u) {
        outColor = texture(vramWrite, vramUv);
    }
}