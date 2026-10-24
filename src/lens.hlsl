// Lens shader. The render target and the desktop texture both cover the whole output 1:1,
// so SV_Position.xy is directly a desktop pixel coordinate.

cbuffer Constants : register(b0)
{
    float4 lens;        // x0, y0, x1, y1 (pixels)
    float4 cursor_zoom; // cursor x, cursor y (pixel centre), zoom, global alpha
    float4 style;       // corner radius, border width, shadow size, unused
    float4 border_rgb;  // border colour (linear-ish sRGB values), unused
    float4 viewport;    // width, height, 1/width, 1/height
};

Texture2D desktop : register(t0);
SamplerState smp : register(s0);

struct VsOut
{
    float4 pos : SV_Position;
};

VsOut vs_main(uint id : SV_VertexID)
{
    // Triangle strip quad covering the lens plus its shadow margin.
    float2 t = float2(id & 1, id >> 1);
    float margin = style.z + 2.0;
    float2 p = lerp(lens.xy - margin, lens.zw + margin, t);
    VsOut o;
    o.pos = float4(p * viewport.zw * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}

float sd_round_box(float2 p, float2 half_size, float r)
{
    float2 q = abs(p) - half_size + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

float4 ps_main(VsOut i) : SV_Target
{
    float2 p = i.pos.xy;
    float2 centre = (lens.xy + lens.zw) * 0.5;
    float2 half_size = (lens.zw - lens.xy) * 0.5;
    float d = sd_round_box(p - centre, half_size, style.x);

    float inside = saturate(0.5 - d);

    // Zoom about the cursor: the desktop point under the cursor stays under the cursor.
    float2 c = cursor_zoom.xy;
    float2 s = c + (p - c) / cursor_zoom.z;
    float3 col = desktop.SampleLevel(smp, s * viewport.zw, 0).rgb;

    float bw = style.y;
    if (bw > 0.0)
    {
        float ring = saturate(d + bw + 0.5);
        col = lerp(col, border_rgb.rgb, ring);
    }

    float shadow = 0.0;
    if (style.z > 0.0)
    {
        float k = saturate(1.0 - d / style.z);
        shadow = 0.35 * k * k;
    }

    float a = inside + shadow * (1.0 - inside);
    // Premultiplied alpha; the shadow is black so it only contributes alpha.
    return float4(col * inside, a) * cursor_zoom.w;
}
