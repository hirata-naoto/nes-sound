#include <stdbool.h>
#include <stdint.h>

#include "hardware/clocks.h"
#include "hardware/pwm.h"
#include "pico/stdlib.h"

#include "nes_apu.h"

#define AUDIO_PWM_GPIO 1
#define AUDIO_PWM_WRAP 255
#define AUDIO_SAMPLE_RATE_HZ 22050
#define TEMPO_TICKS_PER_SECOND 60

typedef struct {
    uint16_t duration_ticks;
    float pulse0_hz;
    float pulse1_hz;
    float triangle_hz;
    float pulse0_volume;
    float pulse1_volume;
    float triangle_volume;
    float pulse0_duty;
    float pulse1_duty;
    bool noise_enabled;
    bool noise_short_mode;
    uint8_t noise_period_index;
    float noise_volume;
} demo_step_t;

static const demo_step_t k_demo_song[] = {
    {12, 523.25f, 783.99f, 130.81f, 0.55f, 0.35f, 0.35f, 0.125f, 0.25f, true, false, 5, 0.20f},
    {12, 659.25f, 987.77f, 164.81f, 0.55f, 0.35f, 0.35f, 0.125f, 0.25f, false, false, 0, 0.00f},
    {12, 783.99f, 1174.66f, 196.00f, 0.55f, 0.35f, 0.35f, 0.250f, 0.50f, true, true, 4, 0.20f},
    {12, 659.25f, 987.77f, 164.81f, 0.55f, 0.35f, 0.35f, 0.125f, 0.25f, false, false, 0, 0.00f},
    {12, 587.33f, 880.00f, 146.83f, 0.55f, 0.35f, 0.35f, 0.125f, 0.25f, true, false, 6, 0.18f},
    {12, 659.25f, 987.77f, 164.81f, 0.55f, 0.35f, 0.35f, 0.250f, 0.50f, false, false, 0, 0.00f},
    {12, 698.46f, 1046.50f, 174.61f, 0.55f, 0.35f, 0.35f, 0.125f, 0.25f, true, true, 5, 0.20f},
    {24, 783.99f, 1174.66f, 196.00f, 0.60f, 0.40f, 0.40f, 0.250f, 0.50f, true, false, 3, 0.16f},
};

static nes_apu_t g_apu;
static volatile uint32_t g_song_step_index = 0;
static volatile uint32_t g_ticks_until_step_change = 0;
static volatile uint32_t g_samples_until_tick = 0;

static void apply_demo_step(const demo_step_t *step) {
    nes_apu_set_pulse(&g_apu, 0, true, step->pulse0_hz, step->pulse0_volume, step->pulse0_duty);
    nes_apu_set_pulse(&g_apu, 1, true, step->pulse1_hz, step->pulse1_volume, step->pulse1_duty);
    nes_apu_set_triangle(&g_apu, true, step->triangle_hz, step->triangle_volume);
    nes_apu_set_noise(&g_apu, step->noise_enabled, step->noise_period_index, step->noise_volume);
    nes_apu_set_noise_mode(&g_apu, step->noise_short_mode);
    g_ticks_until_step_change = step->duration_ticks;
}

static void advance_demo_song_if_needed(void) {
    if (g_samples_until_tick > 0) {
        --g_samples_until_tick;
    }

    if (g_samples_until_tick == 0) {
        g_samples_until_tick = AUDIO_SAMPLE_RATE_HZ / TEMPO_TICKS_PER_SECOND;

        if (g_ticks_until_step_change > 0) {
            --g_ticks_until_step_change;
        }

        if (g_ticks_until_step_change == 0) {
            g_song_step_index = (g_song_step_index + 1u) % (sizeof(k_demo_song) / sizeof(k_demo_song[0]));
            apply_demo_step(&k_demo_song[g_song_step_index]);
        }
    }
}

static bool audio_timer_callback(repeating_timer_t *timer) {
    (void)timer;

    advance_demo_song_if_needed();

    const int16_t sample = nes_apu_next_sample(&g_apu);
    const uint8_t pwm_level = (uint8_t)(((int32_t)sample + 32768) >> 8);
    pwm_set_gpio_level(AUDIO_PWM_GPIO, pwm_level);

    return true;
}

static void setup_audio_pwm(void) {
    gpio_set_function(AUDIO_PWM_GPIO, GPIO_FUNC_PWM);
    const uint slice_num = pwm_gpio_to_slice_num(AUDIO_PWM_GPIO);

    pwm_config config = pwm_get_default_config();
    pwm_config_set_wrap(&config, AUDIO_PWM_WRAP);
    pwm_config_set_clkdiv(&config, 1.0f);
    pwm_init(slice_num, &config, true);
    pwm_set_gpio_level(AUDIO_PWM_GPIO, AUDIO_PWM_WRAP / 2u);
}

int main(void) {
    stdio_init_all();
    setup_audio_pwm();
    nes_apu_init(&g_apu, AUDIO_SAMPLE_RATE_HZ);

    g_song_step_index = 0;
    g_samples_until_tick = AUDIO_SAMPLE_RATE_HZ / TEMPO_TICKS_PER_SECOND;
    apply_demo_step(&k_demo_song[0]);

    repeating_timer_t timer;
    add_repeating_timer_us(-1000000 / AUDIO_SAMPLE_RATE_HZ, audio_timer_callback, NULL, &timer);

    while (true) {
        tight_loop_contents();
    }
}
