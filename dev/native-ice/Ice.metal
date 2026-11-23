#include <metal_stdlib>
using namespace metal;
struct Vertex { float4 position [[position]]; float2 uv; };
struct Params { float time; float dissolve; float aspect; float synthetic; };
vertex Vertex fullscreen(uint id [[vertex_id]]) {
    float2 p = float2((id << 1) & 2, id & 2);
    return {float4(p * float2(2,-2) + float2(-1,1), 0, 1), p};
}
float hash(float2 p) { return fract(sin(dot(p, float2(127.1,311.7))) * 43758.5453); }
float noise(float2 p) {
    float2 i = floor(p), f = fract(p); f = f*f*(3.-2.*f);
    return mix(mix(hash(i),hash(i+float2(1,0)),f.x),mix(hash(i+float2(0,1)),hash(i+1),f.x),f.y);
}
float cloud(float2 p) { return noise(p)*.58 + noise(p*2.13)*.28 + noise(p*4.7)*.14; }
fragment float4 ice(Vertex in [[stage_in]], texture2d<float> scene [[texture(0)]],
                    texture2d<float> frost [[texture(1)]], texture2d<float> touches [[texture(2)]],
                    texture2d<float> softScene [[texture(3)]], constant Params &p [[buffer(0)]]) {
    constexpr sampler s(filter::linear, address::clamp_to_edge);
    float sourceAspect = float(scene.get_width()) / float(scene.get_height());
    float2 fit = p.aspect > sourceAspect ? float2(sourceAspect/p.aspect,1) : float2(1,p.aspect/sourceAspect);
    float2 uv = (in.uv-.5)/fit+.5;
    if (any(uv < 0.) || any(uv > 1.)) return float4(.07,.08,.09,1);
    float2 q = uv*float2(sourceAspect,1);
    float edge = min(min(uv.x,1.-uv.x)*sourceAspect, min(uv.y,1.-uv.y));
    float cloudy = cloud(q*7.1);
    float rim = 1.-smoothstep(.015,.29 + .065*(cloudy-.5),edge);
    float thickness = .46 + .28*cloudy + .36*rim;
    constexpr sampler contactSampler(filter::nearest, address::clamp_to_edge);
    float contact = touches.sample(contactSampler,in.uv).r;
    float thaw = contact > .5 ? smoothstep(0.,.65,p.time-(contact-1.)) : 0.;
    float radius = length((uv-.5)*float2(1.3,1.))/length(float2(.65,.5));
    float wave = radius*.92 + .05*(cloudy-.5);
    float natural = p.dissolve < 0 ? 0. : smoothstep(wave-.10,wave+.10,p.dissolve*1.2-.1);
    float remaining = (1.-thaw)*(1.-natural);
    thickness *= remaining;
    float2 slope = float2(cloud(q*7.1+float2(.025,0))-cloudy,cloud(q*7.1+float2(0,.025))-cloudy);
    float2 refracted = uv + slope*.023*remaining;
    float3 clear = scene.sample(s,uv).rgb;
    float3 blurred = softScene.sample(s,refracted).rgb;
    float3 under = mix(clear,blurred,smoothstep(0.,.85,thickness)*.92);
    float luma = dot(under,float3(.2126,.7152,.0722));
    under = mix(under,mix(float3(luma),float3(.48,.57,.63),.32),.23*remaining);
    float haze = (.18 + .19*cloudy + rim*.12)*remaining;
    float3 result = mix(under,float3(.69,.77,.81),haze);
    float4 crystal = frost.sample(s,uv);
    float crystalAlpha = crystal.a * (.57 + rim*.34) * remaining;
    result = mix(result,crystal.rgb,crystalAlpha);
    float grain = hash(floor(uv*float2(scene.get_width(),scene.get_height())*1.7));
    result += (grain-.5)*(.035+.065*rim)*remaining;
    float fine = smoothstep(.82,.97,noise(q*420.))*rim;
    result += fine*.10*remaining;
    if (p.synthetic > .5 && uv.y > .93 && uv.x > fract(p.time*.055) && uv.x < fract(p.time*.055)+.025)
        result = mix(result,float3(.95,.65,.2),.85);
    return float4(clamp(result,0.,1.),1);
}
