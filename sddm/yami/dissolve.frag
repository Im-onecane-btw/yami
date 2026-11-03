#version 440
// Проявление клетками по матрице Байера 4×4 — как у панелей шелла (gfx.rs dissolve)
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float k;
    float cell;
    vec2 size;
};
layout(binding = 1) uniform sampler2D source;

void main() {
    vec2 p = floor(qt_TexCoord0 * size / cell);
    int x = int(mod(p.x, 4.0));
    int y = int(mod(p.y, 4.0));
    float b[16] = float[16](0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    float th = b[y * 4 + x];
    vec4 c = texture(source, qt_TexCoord0);
    fragColor = (th < floor(k * 16.0)) ? c * qt_Opacity : vec4(0.0);
}
