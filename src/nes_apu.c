#include "nes_apu.h"

#include <math.h>

#define NES_APU_CPU_CLOCK_HZ 1789773.0f
#define NES_APU_DC_BLOCK_COEFFICIENT 0.995f

static const uint8_t k_pulse_duty_sequences[4][8] = {
    {0u, 1u, 0u, 0u, 0u, 0u, 0u, 0u},
    {0u, 1u, 1u, 0u, 0u, 0u, 0u, 0u},
    {0u, 1u, 1u, 1u, 1u, 0u, 0u, 0u},
    {1u, 0u, 0u, 1u, 1u, 1u, 1u, 1u},
};

static const uint8_t k_triangle_sequence[32] = {
    15u, 14u, 13u, 12u, 11u, 10u, 9u, 8u,
    7u, 6u, 5u, 4u, 3u, 2u, 1u, 0u,
    0u, 1u, 2u, 3u, 4u, 5u, 6u, 7u,
    8u, 9u, 10u, 11u, 12u, 13u, 14u, 15u,
};

static const uint16_t k_noise_period_cycles[16] = {
    4u, 8u, 16u, 32u, 64u, 96u, 128u, 160u,
    202u, 254u, 380u, 508u, 762u, 1016u, 2034u, 4068u,
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

static uint8_t quantize_volume(float volume) {
    return (uint8_t)lrintf(clampf(volume, 0.0f, 1.0f) * 15.0f);
}

static uint8_t quantize_duty_mode(float duty_cycle) {
    static const float duty_modes[4] = {0.125f, 0.25f, 0.50f, 0.75f};
    uint8_t best_mode = 0u;
    float best_distance = fabsf(duty_cycle - duty_modes[0]);

    for (uint8_t i = 1u; i < 4u; ++i) {
        const float distance = fabsf(duty_cycle - duty_modes[i]);
        if (distance < best_distance) {
            best_distance = distance;
            best_mode = i;
        }
    }

    return best_mode;
}

static uint16_t quantize_timer_period(float divisor, float frequency_hz) {
    if (frequency_hz <= 0.0f) {
        return 0u;
    }

    float timer = (NES_APU_CPU_CLOCK_HZ / (divisor * frequency_hz)) - 1.0f;
    timer = roundf(clampf(timer, 0.0f, 2047.0f));
    return (uint16_t)timer;
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

    nes_pulse_channel_t *pulse = &apu->pulse[channel];
    pulse->enabled = enabled;
    pulse->frequency_hz = frequency_hz;
    pulse->volume = clampf(volume, 0.0f, 1.0f);
    pulse->duty_cycle = duty_cycle;
    pulse->timer_period = quantize_timer_period(16.0f, frequency_hz);
    pulse->duty_mode = quantize_duty_mode(duty_cycle);
}

void nes_apu_set_triangle(nes_apu_t *apu, bool enabled, float frequency_hz, float volume) {
    apu->triangle.enabled = enabled;
    apu->triangle.frequency_hz = frequency_hz;
    apu->triangle.volume = clampf(volume, 0.0f, 1.0f);
    apu->triangle.timer_period = quantize_timer_period(32.0f, frequency_hz);
}

void nes_apu_set_noise(nes_apu_t *apu, bool enabled, uint8_t period_index, float volume) {
    apu->noise.enabled = enabled;
    apu->noise.period_index = period_index & 0x0f;
    apu->noise.volume = clampf(volume, 0.0f, 1.0f);
}

void nes_apu_set_noise_mode(nes_apu_t *apu, bool short_mode) {
    apu->noise.short_mode = short_mode;
}

static float render_pulse(nes_pulse_channel_t *channel, uint32_t sample_rate_hz) {
    if (!channel->enabled || channel->volume <= 0.0f || channel->frequency_hz <= 0.0f || channel->timer_period < 8u) {
        return 0.0f;
    }

    const float timer_cycles = 16.0f * (float)(channel->timer_period + 1u);
    channel->timer_phase_cycles += NES_APU_CPU_CLOCK_HZ / (float)sample_rate_hz;

    while (channel->timer_phase_cycles >= timer_cycles) {
        channel->timer_phase_cycles -= timer_cycles;
        channel->sequence_index = (uint8_t)((channel->sequence_index + 1u) & 0x07u);
    }

    const uint8_t level = k_pulse_duty_sequences[channel->duty_mode][channel->sequence_index];
    return (float)level * (float)quantize_volume(channel->volume);
}

static float render_triangle(nes_triangle_channel_t *channel, uint32_t sample_rate_hz) {
    if (!channel->enabled || channel->volume <= 0.0f || channel->frequency_hz <= 0.0f) {
        return 0.0f;
    }

    const float timer_cycles = 32.0f * (float)(channel->timer_period + 1u);
    channel->timer_phase_cycles += NES_APU_CPU_CLOCK_HZ / (float)sample_rate_hz;

    while (channel->timer_phase_cycles >= timer_cycles) {
        channel->timer_phase_cycles -= timer_cycles;
        channel->sequence_index = (uint8_t)((channel->sequence_index + 1u) & 0x1fu);
    }

    return (float)k_triangle_sequence[channel->sequence_index] * clampf(channel->volume, 0.0f, 1.0f);
}

static float render_noise(nes_noise_channel_t *channel, uint32_t sample_rate_hz) {
    if (!channel->enabled || channel->volume <= 0.0f) {
        return 0.0f;
    }

    const float timer_cycles = (float)k_noise_period_cycles[channel->period_index];
    channel->timer_phase_cycles += NES_APU_CPU_CLOCK_HZ / (float)sample_rate_hz;

    while (channel->timer_phase_cycles >= timer_cycles) {
        channel->timer_phase_cycles -= timer_cycles;
        const uint16_t tap = channel->short_mode ? 6u : 1u;
        const uint16_t feedback = ((channel->lfsr & 1u) ^ ((channel->lfsr >> tap) & 1u)) & 1u;
        channel->lfsr = (channel->lfsr >> 1u) | (feedback << 14u);
        if (channel->lfsr == 0u) {
            channel->lfsr = 1u;
        }
    }

    return ((channel->lfsr & 1u) == 0u) ? (float)quantize_volume(channel->volume) : 0.0f;
}

int16_t nes_apu_next_sample(nes_apu_t *apu) {
    const float pulse_0 = render_pulse(&apu->pulse[0], apu->sample_rate_hz);
    const float pulse_1 = render_pulse(&apu->pulse[1], apu->sample_rate_hz);
    const float triangle = render_triangle(&apu->triangle, apu->sample_rate_hz);
    const float noise = render_noise(&apu->noise, apu->sample_rate_hz);

    float pulse_mix = 0.0f;
    const float pulse_sum = pulse_0 + pulse_1;
    if (pulse_sum > 0.0f) {
        pulse_mix = 95.88f / ((8128.0f / pulse_sum) + 100.0f);
    }

    float tnd_mix = 0.0f;
    const float tnd_sum = (triangle / 8227.0f) + (noise / 12241.0f);
    if (tnd_sum > 0.0f) {
        tnd_mix = 159.79f / ((1.0f / tnd_sum) + 100.0f);
    }

    const float mixed = pulse_mix + tnd_mix;
    const float filtered = mixed - apu->dc_prev_input + (NES_APU_DC_BLOCK_COEFFICIENT * apu->dc_prev_output);
    apu->dc_prev_input = mixed;
    apu->dc_prev_output = filtered;

    return (int16_t)lrintf(clampf(filtered * 1.5f, -1.0f, 1.0f) * 32767.0f);
}
