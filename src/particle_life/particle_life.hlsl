#define ROOT_SIGNATURE \
    "RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT)," \
    "CBV(b0)," \
    "SRV(t0)," \
    "SRV(t1)," \
    "UAV(u0)," \
    "UAV(u1)"
    

cbuffer CONSTANTS : register(b0) {
    uint ParticleTypeMax;
    uint NumParticles;
    float2 WorldSize;
}

struct Rule {
    float force;
    float min_distance;
    float max_distance;
};

struct Particle {
    float2 position;
    float2 velocity;
    uint type;
    float2 force;
};

struct Vertex {
    float2 position;
    uint color;
};

StructuredBuffer<Rule> Rules : register(t0);
StructuredBuffer<Particle> OldParticles : register(t1);
RWStructuredBuffer<Particle> NewParticles : register(u0);
RWStructuredBuffer<Vertex> Vertices : register(u1);


uint particle_type_to_color(uint type);


[numthreads(32, 1, 1)]
void main(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint particle_id = dispatch_thread_id.x;

    NewParticles[particle_id] = OldParticles[particle_id];
    NewParticles[particle_id].force = float2(0,0);

    Vertices[particle_id].position = NewParticles[particle_id].position;
    Vertices[particle_id].color = particle_type_to_color(NewParticles[particle_id].type);
}



// from https://chilliant.com/rgb2hsv.html
float3 hue2rgb(float H) {
    float R = abs(H * 6 - 3) - 1;  
    float G = 2 - abs(H * 6 - 2);  
    float B = 2 - abs(H * 6 - 4); 
    return saturate(float3(R,G,B));
}

uint particle_type_to_color(uint type) {
    float hue = (float)type / float(ParticleTypeMax);
    float3 rgb = hue2rgb(hue) * 255;

    uint r = rgb.x;
    uint g = rgb.y;
    uint b = rgb.z;
    uint a = 255; 

    return (a << 24) | (b << 16) | (g << 8) | r;
    
}