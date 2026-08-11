#ifndef NES_APU_H
#define NES_APU_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define NES_APU_PULSE_CHANNELS 2

typedef struct {
    bool enabled;
    float frequency_hz;
    float volume;
    float duty_cycle;
    uint16_t timer_period;
    uint8_t duty_mode;
    uint8_t sequence_index;
    float timer_phase_cycles;
} nes_pulse_channel_t;

typedef struct {
    bool enabled;
    float frequency_hz;
    float volume;
    uint16_t timer_period;
    uint8_t sequence_index;
    float timer_phase_cycles;
} nes_triangle_channel_t;

typedef struct {
    bool enabled;
    uint8_t period_index;
    float volume;
    bool short_mode;
    uint16_t lfsr;
    float timer_phase_cycles;
} nes_noise_channel_t;

typedef struct {
    uint32_t sample_rate_hz;
    nes_pulse_channel_t pulse[NES_APU_PULSE_CHANNELS];
    nes_triangle_channel_t triangle;
    nes_noise_channel_t noise;
    float dc_prev_input;
    float dc_prev_output;
} nes_apu_t;

void nes_apu_init(nes_apu_t *apu, uint32_t sample_rate_hz);
void nes_apu_set_pulse(nes_apu_t *apu, size_t channel, bool enabled, float frequency_hz, float volume, float duty_cycle);
void nes_apu_set_triangle(nes_apu_t *apu, bool enabled, float frequency_hz, float volume);
void nes_apu_set_noise(nes_apu_t *apu, bool enabled, uint8_t period_index, float volume);
void nes_apu_set_noise_mode(nes_apu_t *apu, bool short_mode);
int16_t nes_apu_next_sample(nes_apu_t *apu);

#endif
