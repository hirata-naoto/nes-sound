#include <assert.h>
#include <stdint.h>
#include <stdlib.h>

#include "nes_apu.h"

int main(void) {
    nes_apu_t apu;
    nes_apu_init(&apu, 22050);
    nes_apu_set_pulse(&apu, 0, true, 440.0f, 0.7f, 0.125f);
    nes_apu_set_pulse(&apu, 1, true, 660.0f, 0.5f, 0.50f);
    nes_apu_set_triangle(&apu, true, 220.0f, 0.6f);
    nes_apu_set_noise(&apu, true, 4u, 0.2f);

    int64_t sum = 0;
    int16_t previous = 0;
    int transitions = 0;
    int16_t minimum = 32767;
    int16_t maximum = -32768;

    for (int i = 0; i < 4096; ++i) {
        const int16_t sample = nes_apu_next_sample(&apu);
        sum += llabs((long long)sample);
        if (sample < minimum) {
            minimum = sample;
        }
        if (sample > maximum) {
            maximum = sample;
        }
        if (i > 0 && sample != previous) {
            ++transitions;
        }
        previous = sample;
    }

    assert(sum > 1000000);
    assert(transitions > 100);
    assert(minimum < 0);
    assert(maximum > 0);
    return 0;
}
