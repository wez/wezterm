// In-game CRT shader
// Author: sarphiv
// License: CC BY-NC-SA 4.0
// Description:
//   Cursor shader for Ghostty designed to be applied before the accompanying in-game CRT shader.
//   Highlights the cursor whenever it changes (line/block) or moves, and draws a glowing trail for large movements.
//   The glow is colored based on cursor colors with an override for white-grey colors.


// Settings:
// Minimum distance in UV coordinates before drawing trail
// [0, sqrt(2)]
#define TRAIL_MIN_DISTANCE 0.1
// Use override colors when color channel brightness standard deviation is below threshold
// [0, sqrt(2)/3]
#define GLOW_COLOR_OVERRIDE_THRESHOLD 0.1
// Override color for current cursor
// [0, 1]^3
#define GLOW_COLOR_OVERRIDE_CURRENT 0.2, 0.4, 1.0
// Override color for previous cursor
// [0, 1]^3
#define GLOW_COLOR_OVERRIDE_PREVIOUS 0.4, 0.1, 1.0
// Brightness offset for glow effect
// [0, 1]
#define GLOW_COLOR_OFFSET_BRIGHTNESS 0.5
// Time slowdown factor for animations
// x \in R : x > 0
#define TIME_DURATION_FACTOR 1.0



// Constants:
#define EPS 1e-9



// Functions:
float min_(float a, float b, float c) { return min(a, min(b, c)); }
float max_(float a, float b, float c) { return max(a, max(b, c)); }

float sdRectangle(vec2 p, vec2 topLeft, vec2 size) {
    vec2 center = topLeft + vec2(size.x, -size.y) * 0.5;
    vec2 d = abs(p - center) - size * 0.5;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

float sdSeg(vec2 p, vec2 a)
{
    vec2 c = a * clamp(dot(p, a) / (dot(a, a) + EPS), 0., 1.) - p;
    return sqrt(dot(c, c));
}

float sdTriangle(vec2 p, vec2 a, vec2 b, vec2 c)
{
    a -= p; b -= p; c -= p;

    vec3 t = cross(vec3(a.x, b.x, c.x), vec3(a.y, b.y, c.y));
    vec2 m = vec2(min_(t.x, t.y, t.z), max_(t.x, t.y, t.z));
    float s = -1. + 2. * step(m.x, 0.) * step(0., m.y);

    return s * min_(
        sdSeg(a, a - b),
        sdSeg(b, b - c),
        sdSeg(c, c - a)
    );
}

float sdTrail(vec2 p, vec2 currPos, vec2 currSize, vec2 prevPos, vec2 prevSize, float t) {
    // Initialize points
    vec2 currWidth = vec2(currSize.x, 0.0), currHeight = vec2(0.0, -currSize.y);
    vec2 currTopLeft = currPos;
    vec2 currTopRight = currTopLeft + currWidth;
    vec2 currBottomLeft = currTopLeft + currHeight;
    vec2 currBottomRight = currBottomLeft + currWidth;
    vec2 currCenter = (currTopLeft + currBottomRight) * 0.5;

    vec2 prevWidth = vec2(prevSize.x, 0.0), prevHeight = vec2(0.0, -prevSize.y);
    vec2 prevTopLeft = prevPos;
    vec2 prevTopRight = prevTopLeft + prevWidth;
    vec2 prevBottomLeft = prevTopLeft + prevHeight;
    vec2 prevBottomRight = prevBottomLeft + prevWidth;
    vec2 prevCenter = (prevTopLeft + prevBottomRight) * 0.5;

    // Check whether to only draw cursor
    bool nearbyPrev = distance(currCenter, prevCenter) < TRAIL_MIN_DISTANCE;
    bool insidePrev = (
        currCenter.x >= prevTopLeft.x && currCenter.x <= prevTopRight.x &&
        currCenter.y <= prevTopLeft.y && currCenter.y >= prevBottomLeft.y
    );

    float rectDist = max(sdRectangle(p, currTopLeft, currSize), 0.0);

    if (nearbyPrev || insidePrev)
        return rectDist;

    // Draw trail and cursor
    vec2[4] corners = { currTopLeft, currTopRight, currBottomRight, currBottomLeft };
    vec2 triB = corners[0], triC = corners[0], dir = normalize(currCenter - prevCenter);
    float minRel = 1/EPS, maxRel = -minRel;

    for (int i = 0; i < 4; ++i) {
        vec2 delta = corners[i] - prevCenter;
        float rel = atan(dir.x * delta.y - dir.y * delta.x, dot(dir, delta));

        if (rel < minRel)
            minRel = rel, triB = corners[i];
        if (rel > maxRel)
            maxRel = rel, triC = corners[i];
    }

    float triDist = max(sdTriangle(p, prevCenter, triB, triC), 0.0);


    return min(rectDist, mix(triDist, rectDist, t));
}

vec4 colorOverride(vec4 baseColor, vec4 overrideColor) {
    if (sqrt(pow(distance(baseColor.rgb, vec3(dot(baseColor.rgb, vec3(1.0)) / 3.0)), 2) / 3.0) < GLOW_COLOR_OVERRIDE_THRESHOLD)
        return overrideColor;
    else
        return baseColor;
}


void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;
    fragColor = texture(iChannel0, uv);

    vec2 currPos = iCurrentCursor.xy / iResolution.xy, currSize = iCurrentCursor.zw / iResolution.xy;
    vec2 prevPos = iPreviousCursor.xy / iResolution.xy, prevSize = iPreviousCursor.zw / iResolution.xy;
    vec2 currCenter = currPos + currSize * vec2(0.5, -0.5);
    vec2 prevCenter = prevPos + prevSize * vec2(0.5, -0.5);

    float dCenter = distance(currCenter, prevCenter);
    float dSeg = dot(uv - prevCenter, currCenter - prevCenter) * pow(dCenter + EPS, -2);
    bool nearbyPrev = dCenter < TRAIL_MIN_DISTANCE;

    float tShape = 1.0 - pow(1.0 - clamp((iTime - iTimeCursorChange) / TIME_DURATION_FACTOR, 0.0, 1.0), 3);
    float tVisible = exp(-(iTime - iTimeCursorChange) / TIME_DURATION_FACTOR * 50.0);
    
    float dTrail = sdTrail(uv, currPos, currSize, prevPos, prevSize, tShape);
    float dTip = nearbyPrev ? 0.0 : clamp(1.0 - abs(dSeg - 1.0), 0.0, 1.0);

    vec4 currColor = colorOverride(iCurrentCursorColor, vec4(GLOW_COLOR_OVERRIDE_CURRENT, 1.0));
    vec4 prevColor = colorOverride(iPreviousCursorColor, vec4(GLOW_COLOR_OVERRIDE_PREVIOUS, 1.0));
    vec4 glowColor = mix(fragColor, mix(prevColor, currColor, dTip) + GLOW_COLOR_OFFSET_BRIGHTNESS, pow(dTip, 3));
    glowColor = mix(glowColor, fragColor, pow(smoothstep(0.0, 0.3, dTrail), 0.1));

    vec4 trailColor = mix(vec4(1.0), glowColor, pow(smoothstep(0.0, 0.01, dTrail), 0.2));
    vec4 trail = mix(trailColor, fragColor, pow(smoothstep(0.0, nearbyPrev ? 0.01 : 0.1, dTrail), 0.2));
    if (!nearbyPrev) {
        trail = mix(trailColor, trail, pow(smoothstep(0.0, 6.0, dTip), 0.05));
        trail = mix(trailColor, trail, pow(smoothstep(0.0, 8.0, dTip), 0.005));
    }

    fragColor = mix(fragColor, trail, tVisible);
}
