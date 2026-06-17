#version 300 es
precision highp float;
precision highp usampler2D;

uniform sampler2D vramWrite;
uniform usampler2D vramRead;
uniform uint displayDepth;
uniform uvec2 displayStart;
uniform uvec2 displaySize;

in vec2 vUv;
out vec4 outColor;

uint readByte(uint byteOffset, uint y) {
    uint halfwordX = byteOffset / 2u;
    uint halfword = texelFetch(vramRead, ivec2(halfwordX, y), 0).r;

    if ((byteOffset & 1u) == 1u) {
        return halfword >> 8u;
    }

    return halfword & 0xffu;
}

vec4 readPixel24bit(uint displayStartX, uint displayPixelX, uint y) {
    uint byteOffset = displayStartX * 2u + displayPixelX * 3u;

    uint r = readByte(byteOffset + 0u, y);
    uint g = readByte(byteOffset + 1u, y);
    uint b = readByte(byteOffset + 2u, y);

    return vec4(r, g, b, 255u) / 255.0;
}

void main() {
    vec2 vramUv = vec2(0u, 0u);
    vramUv.x = (float(displayStart.x) + vUv.x * float(displaySize.x)) / 1024.0;
    vramUv.y = (float(512u - displayStart.y - displaySize.y) + vUv.y * float(displaySize.y)) / 512.0;

    if (displayDepth == 0u) {
        outColor = texture(vramWrite, vramUv);
    } else {
        uint srcY = displayStart.y + uint((1.0 - vUv.y) * float(displaySize.y));
        uint displayX = min(
            uint(vUv.x * float(displaySize.x)),
            displaySize.x - 1u
        );

        outColor = readPixel24bit(
            displayStart.x,
            displayX,
            srcY
        );
    }
}