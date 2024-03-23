struct VS_INPUT {
    float2 position: POSITION;
    float4 color: COLOR0;
};

struct PS_INPUT {
    float4 position: POSITION;
    float4 color: COLOR0;
};


PS_INPUT vs_main(VS_INPUT input) {
    PS_INPUT output;

    output.position = float4(input.position.x, input.position.y, 0, 0);
    output.color = input.color;

    return output;
}



float4 ps_main(PS_INPUT input): SV_TARGET {
    return input.color;
}