#define ROOT_SIGNATURE \
    "RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT)," \
    "RootConstants(num32BitConstants=16, b0, visibility=SHADER_VISIBILITY_VERTEX)"


cbuffer VS_CONSTANTS : register(b0) {
    float4x4 ProjectionMatrix;
}

struct VS_INPUT {
    float2 position: POSITION;
    float4 color: COLOR0;
};

struct PS_INPUT {
    float4 position: SV_POSITION;
    float4 color: COLOR0;
};

PS_INPUT vs_main(VS_INPUT input) {
    PS_INPUT output;

    output.position = mul(
        ProjectionMatrix, 
        float4(input.position.xy, 0.0f, 1.0f));

    output.color = input.color;

    return output;
}

float4 ps_main(PS_INPUT input): SV_TARGET {
    return input.color;
}