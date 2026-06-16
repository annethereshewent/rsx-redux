#version 300 es
precision highp float;
precision highp usampler2D;

in vec4 vColor;
out vec4 outColor;

in vec2 vUv;

uniform usampler2D vramRead;
uniform bool hasTexture;
uniform bool semitransparent;
uniform bool modulate;
uniform uint textureMaskX;
uniform uint textureMaskY;
uniform uint textureOffsetX;
uniform uint textureOffsetY;
uniform int depth;
uniform uint transparentMode;
uniform uvec2 page;
uniform uvec2 clut;
uniform bool forceMaskBit;
uniform bool preserveMaskedPixels;

vec4 getTexColor4bpp(usampler2D vramRead) {
    uint u = uint(vUv.x) & 0xffu;
    uint v = uint(vUv.y) & 0xffu;

    u = (u & ~textureMaskX) | (textureOffsetX & textureMaskX);
    v = (v & ~textureMaskY) | (textureOffsetY & textureMaskY);

    uint offsetU = page.x + u / 4u;
    uint offsetV = page.y + v;

    uint halfword = texelFetch(vramRead, ivec2(offsetU, offsetV), 0).r;

    uint texelIndex = ((u >> 1u) & 1u) == 0u ? halfword & 0xffu : (halfword >> 8u) & 0xffu;

    if ((u & 1u) == 0u) {
        texelIndex &= 0xfu;
    } else {
        texelIndex = (texelIndex >> 4u) & 0xfu;
    }

    uint texel = texelFetch(vramRead, ivec2(texelIndex + clut.x, clut.y), 0).r;

    if (texel == 0u) {
        discard;
    }

    uint r = texel & 0x1fu;
    uint g = (texel >> 5u) & 0x1fu;
    uint b = (texel >> 10u) & 0x1fu;
    uint a = (texel >> 15u) & 1u;

    return vec4(r, g, b, a) / 31.0;
}

vec4 getTexColor8bpp(usampler2D vramRead) {
    return vec4(0, 1, 0, 1);
}

vec4 getTexColor15bpp(usampler2D vramRead) {
    return vec4(1, 0, 0, 1);
}

void main() {
    outColor = vColor;
    if (hasTexture) {
        switch (depth) {
            case 0:
                outColor = getTexColor4bpp(vramRead);
                break;
            case 1:
                outColor = getTexColor8bpp(vramRead);
                break;
            case 2:
                outColor = getTexColor15bpp(vramRead);
                break;
        }
    }
}