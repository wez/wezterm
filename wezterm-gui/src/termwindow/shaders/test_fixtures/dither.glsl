// Simple "dithering" effect
// (c) moni-dz (https://github.com/moni-dz)
// CC BY-NC-SA 4.0 (https://creativecommons.org/licenses/by-nc-sa/4.0/)

// Packed bayer pattern using bit manipulation
const float bayerPattern[4] = float[4](
    0x0514, // Encoding 0,8,2,10
    0xC4E6, // Encoding 12,4,14,6
    0x3B19, // Encoding 3,11,1,9
    0xF7D5  // Encoding 15,7,13,5
);

float getBayerFromPacked(int x, int y) {
    int idx = (x & 3) + ((y & 3) << 2);
    return float((int(bayerPattern[y & 3]) >> ((x & 3) << 2)) & 0xF) * (1.0 / 16.0);
}

#define LEVELS 2.0 // Available color steps per channel
#define INV_LEVELS (1.0 / LEVELS)

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord * (1.0 / iResolution.xy);
    vec3 color = texture(iChannel0, uv).rgb;
 
    float threshold = getBayerFromPacked(int(fragCoord.x), int(fragCoord.y));
    vec3 dithered = floor(color * LEVELS + threshold) * INV_LEVELS;

    fragColor = vec4(dithered, 1.0);
}
