#version 300 es
precision highp float;

uniform sampler2D vramWrite;
uniform uint displayDepth;
uniform uvec2 displayStart;
uniform uvec2 displaySize;

in vec2 vUv;
out vec4 outColor;

void main() {
    vec2 vramUv = (vec2(displayStart) + vUv * vec2(displaySize)) / vec2(1024.0, 512.0);

    if (displayDepth == 0u) {
        outColor = texture(vramWrite, vramUv);
    }
}