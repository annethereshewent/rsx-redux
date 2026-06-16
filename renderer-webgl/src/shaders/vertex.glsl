#version 300 es

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;
layout(location = 3) in vec2 orig;

out vec4 vColor;
out vec2 vUv;

void main() {
    vColor = color;
    vUv = uv;
    gl_Position = vec4(position, 0.0, 1.0);
}