#version 300 es
precision highp float;
precision highp usampler2D;

in vec4 vColor;
out vec4 outColor;

uniform usampler2D vramRead;

void main() {
    outColor = vColor;
}