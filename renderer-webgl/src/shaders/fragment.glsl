#version 300 es
precision highp float;
precision highp usampler2D;

in vec4 vColor;
out vec4 outColor;

in vec2 vUv;
in vec2 vOrig;

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
    uint a = ((texel >> 15u) & 1u) * 0x1fu;

    return vec4(r, g, b, a) / 31.0;
}

vec4 getTexColor8bpp(usampler2D vramRead) {
    uint u = uint(vUv.x) & 0xffu;
    uint v = uint(vUv.y) & 0xffu;

    u = (u & ~textureMaskX) | (textureOffsetX & textureMaskX);
    v = (v & ~textureMaskY) | (textureOffsetY & textureMaskY);

    uint offsetU = page.x + u / 2u;
    uint offsetV = page.y + v;

    uint halfword = texelFetch(vramRead, ivec2(offsetU, offsetV), 0).r;

    uint texelIndex = (u & 1u) == 0u ? halfword & 0xffu : (halfword >> 8u) & 0xffu;

    uint texel = texelFetch(vramRead, ivec2(texelIndex + clut.x, clut.y), 0).r;

    if (texel == 0u) {
        discard;
    }

    uint r = texel & 0x1fu;
    uint g = (texel >> 5u) & 0x1fu;
    uint b = (texel >> 10u) & 0x1fu;
    uint a = ((texel >> 15u) & 1u) * 0x1fu;

    return vec4(r, g, b, a) / 31.0;
}

vec4 getTexColor15bpp(usampler2D vramRead) {
    uint u = uint(vUv.x) & 0xffu;
    uint v = uint(vUv.y) & 0xffu;

    u = (u & ~textureMaskX) | (textureOffsetX & textureMaskX);
    v = (v & ~textureMaskY) | (textureOffsetY & textureMaskY);

    uint offsetU = page.x + u;
    uint offsetV = page.y + v;

    uint texel = texelFetch(vramRead, ivec2(offsetU, offsetV), 0).r;

    if (texel == 0u) {
        discard;
    }

    uint r = texel & 0x1fu;
    uint g = (texel >> 5u) & 0x1fu;
    uint b = (texel >> 10u) & 0x1fu;
    uint a = ((texel >> 15u) & 1u) * 0x1fu;

    return vec4(r, g, b, a) / 31.0;
}

vec4 getOldColor() {
    uint oldTexel = texelFetch(vramRead, ivec2(vOrig), 0).r;

    uint r = oldTexel & 0x1fu;
    uint g = (oldTexel >> 5u) & 0x1fu;
    uint b = (oldTexel >> 10u) & 0x1fu;
    uint a = ((oldTexel >> 15u) & 1u) * 0x1fu;

    return vec4(r, g, b, a) / 31.0;
}

void main() {
    outColor = vColor;
    float texAlpha = 0.0;

    if (preserveMaskedPixels) {
        vec4 old = getOldColor();

        if (old.a != 0.0) {
            discard;
        }
    }

    if (hasTexture) {
        vec4 texColor;
        switch (depth) {
            case 0:
                texColor = getTexColor4bpp(vramRead);
                break;
            case 1:
                texColor = getTexColor8bpp(vramRead);
                break;
            case 2:
                texColor = getTexColor15bpp(vramRead);
                break;
        }

        texAlpha = texColor[3];

        if (modulate) {
            uvec4 texColorUint = uvec4(texColor * 255.0);
            uvec4 vertColorUint = uvec4(vColor * 255.0);

            texColorUint = min((texColorUint * vertColorUint) >> 7u, 0xffu);

            texColor = vec4(texColorUint) / 255.0;
        }

        outColor = texColor;
    }

    if (semitransparent && (!hasTexture || texAlpha == 1.0)) {
        vec4 old = getOldColor();

        switch (transparentMode) {
            case 0u:
                outColor = min((old + outColor) / 2.0, 1.0);
                break;
            case 1u:
                outColor = min(old + outColor, 1.0);
                break;
            case 2u:
                outColor = max(old - outColor, 0.0);
                break;
            case 3u:
                outColor = min(old + (outColor / 4.0), 1.0);
                break;
        }
    }

    if (forceMaskBit) {
        outColor.a = 1.0;
    }
}