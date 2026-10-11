
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = fragCoord/iResolution.xy;
    vec4 color = texture(iChannel0, uv);
    fragColor = vec4(1.0 - color.x, 1.0 - color.y, 1.0 - color.z, color.w);
}

