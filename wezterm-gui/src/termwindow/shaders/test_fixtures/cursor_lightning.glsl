// Cursor lightning trail for Ghostty.
// Draws a brief, jagged bolt from the previous cursor position to the current one.

// -- CONFIGURATION --
const vec3 BOLT_COLOR = vec3(1.00, 0.82, 0.72);
const vec3 GLOW_COLOR = vec3(0.74, 0.00, 0.00);
const float DURATION = 0.14;              // lifetime of each zap, in seconds
const float THRESHOLD_MIN_DISTANCE = 1.5; // cursor widths before a zap is drawn
const float JAGGEDNESS = 0.085;           // sideways displacement relative to bolt length
const float CORE_WIDTH = 1.15;            // pixels
const float GLOW_WIDTH = 5.5;             // pixels
const int BOLT_SEGMENTS = 12;

float hash11(float p) {
    p = fract(p * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

float segmentDistance(vec2 p, vec2 a, vec2 b) {
    vec2 ab = b - a;
    float denom = max(dot(ab, ab), 0.0000001);
    float t = clamp(dot(p - a, ab) / denom, 0.0, 1.0);
    return length(p - (a + ab * t));
}

// A point on a piecewise-linear bolt. Endpoints stay fixed while interior
// nodes are displaced perpendicular to the cursor's direction of travel.
vec2 boltPoint(vec2 a, vec2 b, float t, float seed) {
    vec2 delta = b - a;
    float boltLength = length(delta);
    vec2 perpendicular = vec2(-delta.y, delta.x) / max(boltLength, 0.00001);
    float node = floor(t * float(BOLT_SEGMENTS) + 0.5);
    float randomOffset = hash11(node * 17.17 + seed) * 2.0 - 1.0;
    float endpointFade = sin(3.14159265 * t);
    return mix(a, b, t)
        + perpendicular * randomOffset * boltLength * JAGGEDNESS * endpointFade;
}

float mainBoltDistance(vec2 p, vec2 a, vec2 b, float seed) {
    float distanceToBolt = 100000.0;
    vec2 previous = a;

    for (int i = 1; i <= BOLT_SEGMENTS; ++i) {
        float t = float(i) / float(BOLT_SEGMENTS);
        vec2 current = boltPoint(a, b, t, seed);
        distanceToBolt = min(distanceToBolt, segmentDistance(p, previous, current));
        previous = current;
    }

    return distanceToBolt;
}

// Three small forks growing out of alternating points on the main bolt.
float forkDistance(vec2 p, vec2 a, vec2 b, float seed) {
    float distanceToFork = 100000.0;
    vec2 delta = b - a;
    float boltLength = length(delta);
    vec2 perpendicular = vec2(-delta.y, delta.x) / max(boltLength, 0.00001);

    for (int i = 0; i < 3; ++i) {
        float fi = float(i);
        float t = 0.28 + fi * 0.22;
        vec2 root = boltPoint(a, b, t, seed);
        float side = hash11(seed + fi * 41.0) < 0.5 ? -1.0 : 1.0;
        float reach = boltLength * (0.055 + 0.035 * hash11(seed + fi * 53.0));
        vec2 along = normalize(delta) * reach * (hash11(seed + fi * 67.0) - 0.35);
        vec2 tip = root + perpendicular * side * reach + along;
        vec2 elbow = mix(root, tip, 0.52)
            - perpendicular * side * reach * (0.12 + 0.18 * hash11(seed + fi * 79.0));
        distanceToFork = min(distanceToFork, segmentDistance(p, root, elbow));
        distanceToFork = min(distanceToFork, segmentDistance(p, elbow, tip));
    }

    return distanceToFork;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel0, uv);
    fragColor = terminal;

    vec2 currentCenter = iCurrentCursor.xy
        + vec2(iCurrentCursor.z * 0.5, -iCurrentCursor.w * 0.5);
    vec2 previousCenter = iPreviousCursor.xy
        + vec2(iPreviousCursor.z * 0.5, -iPreviousCursor.w * 0.5);
    float travel = length(currentCenter - previousCenter);
    float minimumTravel = iCurrentCursor.z * THRESHOLD_MIN_DISTANCE;

    float age = clamp((iTime - iTimeCursorChange) / DURATION, 0.0, 1.0);
    if (travel > minimumTravel && age < 1.0) {
        // Quantizing the event time gives each cursor jump a stable bolt shape.
        float seed = floor(iTimeCursorChange * 1000.0) * 0.013;
        float mainDistance = mainBoltDistance(fragCoord, previousCenter, currentCenter, seed);
        float branches = forkDistance(fragCoord, previousCenter, currentCenter, seed);

        float pixel = max(fwidth(mainDistance), 0.65);
        float core = 1.0 - smoothstep(CORE_WIDTH, CORE_WIDTH + pixel, mainDistance);
        float forkCore = 1.0 - smoothstep(CORE_WIDTH * 0.72,
                                          CORE_WIDTH * 0.72 + pixel, branches);
        float glow = exp(-mainDistance * mainDistance / (GLOW_WIDTH * GLOW_WIDTH));
        float forkGlow = exp(-branches * branches / (GLOW_WIDTH * GLOW_WIDTH * 0.55));

        // Lightning flashes hard, then rapidly loses its core and leaves a glow.
        float flash = 1.0 - smoothstep(0.0, 1.0, age);
        flash *= flash;
        float flicker = 0.82 + 0.18 * sin(age * 95.0 + seed);
        float glowAmount = max(glow, forkGlow * 0.55) * flash * flicker * 0.72;
        float coreAmount = max(core, forkCore * 0.82) * flash * flicker;

        vec3 color = mix(fragColor.rgb, GLOW_COLOR, clamp(glowAmount, 0.0, 1.0));
        color = mix(color, BOLT_COLOR, clamp(coreAmount, 0.0, 1.0));
        fragColor = vec4(color, terminal.a);

        // Keep Ghostty's actual cursor crisp above the effect.
        vec2 cursorMin = vec2(iCurrentCursor.x, iCurrentCursor.y - iCurrentCursor.w);
        vec2 cursorMax = vec2(iCurrentCursor.x + iCurrentCursor.z, iCurrentCursor.y);
        float insideCursor = step(cursorMin.x, fragCoord.x)
            * step(cursorMin.y, fragCoord.y)
            * step(fragCoord.x, cursorMax.x)
            * step(fragCoord.y, cursorMax.y);
        fragColor = mix(fragColor, terminal, insideCursor);
    }
}
