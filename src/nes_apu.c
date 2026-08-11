#include "nes_apu.h"

#include <math.h>

static const float k_noise_period_hz[] = {
    440.0f, 660.0f, 880.0f, 1100.0f,
    1320.0f, 1760.0f, 2200.0f, 2640.0f,
    3520.0f, 4400.0f, 5280.0f, 7040.0f,
    8800.0f, 10560.0f, 14080.0f, 17600.0f,
};

static float clampf(float value, float minimum, float maximum) {
    if (value < minimum) {
        return minimum;
    }
    if (value > maximum) {
        return maximum;
    }
    return value;
}

void nes_apu_init(nes_apu_t *apu, uint32_t sample_rate_hz) {
    *apu = (nes_apu_t){
        .sample_rate_hz = sample_rate_hz,
        .noise = {
            .lfsr = 1u,
        },
    };
}

void nes_apu_set_pulse(nes_apu_t *apu, size_t channel, bool enabled, float frequency_hz, float volume, float duty_cycle) {
    if (channel >= NES_APU_PULSE_CHANNELS) {
        return;
    }

    apu->pulse[channel].enabled = enabled;
    apu->pulse[channel].frequency_hz = frequency_hz;
    apu->pulse[channel].volume = clampf(volume, 0.0f, 1.0f);
    apu->pulse[channel].duty_cycle = clampf(duty_cycle, 0.05f, 0.95f);
}

void nes_apu_set_triangle(nes_apu_t *apu, bool enabled, float frequency_hz, float volume) {
    apu->triangle.enabled = enabled;
    apu->triangle.frequency_hz = frequency_hz;
    apu->triangle.volume = clampf(volume, 0.0f, 1.0f);
}

void nes_apu_set_noise(nes_apu_t *apu, bool enabled, uint8_t period_index, float volume) {
    apu->noise.enabled = enabled;
    apu->noise.period_index = period_index & 0x0f;
    apu->noise.volume = clampf(volume, 0.0f, 1.0f);
}

static float render_pulse(nes_pulse_channel_t *channel, uint32_t sample_rate_hz) {
    if (!channel->enabled || channel->volume <= 0.0f || channel->frequency_hz <= 0.0f) {
        return 0.0f;
    }

    channel->phase += channel->frequency_hz / (float)sample_rate_hz;
    channel->phase -= floorf(channel->phase);

    const float step = (channel->phase < channel->duty_cycle) ? 1.0f : -1.0f;
    return 15.0f * channel->volume * step;
}

static float render_triangle(nes_triangle_channel_t *channel, uint32_t sample_rate_hz) {
    if (!channel->enabled || channel->volume <= 0.0f || channel->frequency_hz <= 0.0f) {
        return 0.0f;
    }

    channel->phase += channel->frequency_hz / (float)sample_rate_hz;
    channel->phase -= floorf(channel->phase);

    const float triangle = 1.0f - 4.0f * fabsf(channel->phase - 0.5f);
    return 15.0f * channel->volume * triangle;
}

static float render_noise(nes_noise_channel_t *channel, uint32_t sample_rate_hz) {
    if (!channel->enabled || channel->volume <= 0.0f) {
        return 0.0f;
    }

    const float noise_hz = k_noise_period_hz[channel->period_index];
    channel->phase += noise_hz / (float)sample_rate_hz;

    while (channel->phase >= 1.0f) {
        channel->phase -= 1.0f;
        const uint16_t feedback = (channel->lfsr ^ (channel->lfsr >> 1u)) & 1u;
        channel->lfsr = (channel->lfsr >> 1u) | (feedback << 14u);
        if (channel->lfsr == 0u) {
            channel->lfsr = 1u;
        }
    }

    const float sample = (channel->lfsr & 1u) ? 1.0f : -1.0f;
    return 15.0f * channel->volume * sample;
}

int16_t nes_apu_next_sample(nes_apu_t *apu) {
    const float pulse_0 = render_pulse(&apu->pulse[0], apu->sample_rate_hz);
    const float pulse_1 = render_pulse(&apu->pulse[1], apu->sample_rate_hz);
    const float triangle = render_triangle(&apu->triangle, apu->sample_rate_hz);
    const float noise = render_noise(&apu->noise, apu->sample_rate_hz);

    float pulse_mix = 0.0f;
    const float pulse_sum = fabsf(pulse_0) + fabsf(pulse_1);
    if (pulse_sum > 0.0f) {
        pulse_mix = 95.88f / ((8128.0f / pulse_sum) + 100.0f);
        pulse_mix *= (pulse_0 + pulse_1) / pulse_sum;
    }

    float tnd_mix = 0.0f;
    const float tnd_sum = (fabsf(triangle) / 8227.0f) + (fabsf(noise) / 12241.0f);
    if (tnd_sum > 0.0f) {
        tnd_mix = 159.79f / ((1.0f / tnd_sum) + 100.0f);
        const float signed_mix = triangle + noise;
        const float magnitude = fabsf(triangle) + fabsf(noise);
        if (magnitude > 0.0f) {
            tnd_mix *= signed_mix / magnitude;
        }
    }

    const float mixed = clampf((pulse_mix + tnd_mix) * 1.8f, -1.0f, 1.0f);
    return (int16_t)lrintf(mixed * 32767.0f);
}
