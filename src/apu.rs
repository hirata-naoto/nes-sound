const NES_CPU_CLOCK_HZ: f32 = 1_789_773.0;
const DC_BLOCK_COEFFICIENT: f32 = 0.995;

const PULSE_DUTY_SEQUENCES: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];
const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];
const NOISE_PERIOD_CYCLES: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];
const DPCM_PERIOD_CYCLES: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

#[derive(Clone, Copy, Default)]
struct PulseChannel {
    enabled: bool,
    frequency_hz: f32,
    volume: f32,
    timer_period: u16,
    duty_mode: u8,
    sequence_index: u8,
    timer_phase_cycles: f32,
}

#[derive(Clone, Copy, Default)]
struct TriangleChannel {
    enabled: bool,
    frequency_hz: f32,
    volume: f32,
    timer_period: u16,
    sequence_index: u8,
    timer_phase_cycles: f32,
}

#[derive(Clone, Copy)]
struct NoiseChannel {
    enabled: bool,
    period_index: u8,
    volume: f32,
    short_mode: bool,
    lfsr: u16,
    timer_phase_cycles: f32,
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self {
            enabled: false,
            period_index: 0,
            volume: 0.0,
            short_mode: false,
            lfsr: 1,
            timer_phase_cycles: 0.0,
        }
    }
}

struct DpcmChannel {
    enabled: bool,
    sample: &'static [u8],
    rate_index: u8,
    looping: bool,
    byte_index: usize,
    shift_register: u8,
    bits_remaining: u8,
    output_level: u8,
    output_active: bool,
    timer_phase_cycles: f32,
}

impl Default for DpcmChannel {
    fn default() -> Self {
        Self {
            enabled: false,
            sample: &[],
            rate_index: 0,
            looping: false,
            byte_index: 0,
            shift_register: 0,
            bits_remaining: 0,
            output_level: 64,
            output_active: false,
            timer_phase_cycles: 0.0,
        }
    }
}

/// Software model of the five channels in the NES APU.
pub struct Apu {
    sample_rate_hz: u32,
    pulse: [PulseChannel; 2],
    triangle: TriangleChannel,
    noise: NoiseChannel,
    dpcm: DpcmChannel,
    dc_prev_input: f32,
    dc_prev_output: f32,
}

impl Apu {
    /// Create an APU renderer at the requested audio sample rate.
    pub fn new(sample_rate_hz: u32) -> Self {
        Self {
            sample_rate_hz: sample_rate_hz.max(1),
            pulse: [PulseChannel::default(); 2],
            triangle: TriangleChannel::default(),
            noise: NoiseChannel::default(),
            dpcm: DpcmChannel::default(),
            dc_prev_input: 0.0,
            dc_prev_output: 0.0,
        }
    }

    /// Configure one of the two pulse channels.
    pub fn set_pulse(
        &mut self,
        channel: usize,
        enabled: bool,
        frequency_hz: f32,
        volume: f32,
        duty_cycle: f32,
    ) {
        if channel >= self.pulse.len() {
            return;
        }

        let duty_modes = [0.125, 0.25, 0.5, 0.75];
        let mut duty_mode = 0;
        let mut best_distance = (duty_cycle - duty_modes[0]).abs();
        for (index, mode) in duty_modes.iter().enumerate().skip(1) {
            let distance = (duty_cycle - mode).abs();
            if distance < best_distance {
                best_distance = distance;
                duty_mode = index as u8;
            }
        }

        let pulse = &mut self.pulse[channel];
        pulse.enabled = enabled;
        pulse.frequency_hz = frequency_hz;
        pulse.volume = volume.clamp(0.0, 1.0);
        pulse.timer_period = timer_period(16.0, frequency_hz);
        pulse.duty_mode = duty_mode;
    }

    /// Configure the triangle channel.
    pub fn set_triangle(&mut self, enabled: bool, frequency_hz: f32, volume: f32) {
        self.triangle.enabled = enabled;
        self.triangle.frequency_hz = frequency_hz;
        self.triangle.volume = volume.clamp(0.0, 1.0);
        self.triangle.timer_period = timer_period(32.0, frequency_hz);
    }

    /// Configure the noise channel.
    pub fn set_noise(&mut self, enabled: bool, period_index: u8, volume: f32) {
        self.noise.enabled = enabled;
        self.noise.period_index = period_index & 0x0f;
        self.noise.volume = volume.clamp(0.0, 1.0);
    }

    /// Select the long (false) or short (true) noise LFSR sequence.
    pub fn set_noise_mode(&mut self, short_mode: bool) {
        self.noise.short_mode = short_mode;
    }

    /// Start a DPCM sample from static memory at one of the 16 NES timer rates.
    ///
    /// The sample is played least-significant bit first. Looping restarts it when
    /// the final byte has been consumed; an empty sample is ignored.
    pub fn start_dpcm(&mut self, sample: &'static [u8], rate_index: u8, looping: bool) {
        if sample.is_empty() {
            self.stop_dpcm();
            return;
        }

        self.dpcm.enabled = true;
        self.dpcm.sample = sample;
        self.dpcm.rate_index = rate_index & 0x0f;
        self.dpcm.looping = looping;
        self.dpcm.byte_index = 0;
        self.dpcm.shift_register = 0;
        self.dpcm.bits_remaining = 0;
        self.dpcm.output_level = 64;
        self.dpcm.output_active = true;
        self.dpcm.timer_phase_cycles = 0.0;
    }

    /// Stop fetching DPCM sample bits while retaining the current DAC output.
    pub fn stop_dpcm(&mut self) {
        self.dpcm.enabled = false;
    }

    /// Render and return the next signed 16-bit audio sample.
    pub fn next_sample(&mut self) -> i16 {
        let sample_rate = self.sample_rate_hz;
        let pulse_0 = render_pulse(&mut self.pulse[0], sample_rate);
        let pulse_1 = render_pulse(&mut self.pulse[1], sample_rate);
        let triangle = render_triangle(&mut self.triangle, sample_rate);
        let noise = render_noise(&mut self.noise, sample_rate);
        let dpcm = render_dpcm(&mut self.dpcm, sample_rate);

        let pulse_sum = pulse_0 + pulse_1;
        let pulse_mix = if pulse_sum > 0.0 {
            95.88 / ((8128.0 / pulse_sum) + 100.0)
        } else {
            0.0
        };

        let tnd_sum = (triangle / 8227.0) + (noise / 12241.0) + (dpcm / 22638.0);
        let tnd_mix = if tnd_sum > 0.0 {
            159.79 / ((1.0 / tnd_sum) + 100.0)
        } else {
            0.0
        };

        let mixed = pulse_mix + tnd_mix;
        let filtered = mixed - self.dc_prev_input + (DC_BLOCK_COEFFICIENT * self.dc_prev_output);
        self.dc_prev_input = mixed;
        self.dc_prev_output = filtered;

        round_to_i32(filtered.clamp(-1.0, 1.0) * 1.5 * 32767.0) as i16
    }
}

fn timer_period(divisor: f32, frequency_hz: f32) -> u16 {
    if frequency_hz <= 0.0 {
        return 0;
    }

    ((NES_CPU_CLOCK_HZ / (divisor * frequency_hz) - 1.0).clamp(0.0, 2047.0) + 0.5) as u16
}

fn quantize_volume(volume: f32) -> u8 {
    (volume.clamp(0.0, 1.0) * 15.0 + 0.5) as u8
}

fn round_to_i32(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}

fn render_pulse(channel: &mut PulseChannel, sample_rate_hz: u32) -> f32 {
    if !channel.enabled
        || channel.volume <= 0.0
        || channel.frequency_hz <= 0.0
        || channel.timer_period < 8
    {
        return 0.0;
    }

    let timer_cycles = 16.0 * (channel.timer_period as f32 + 1.0);
    channel.timer_phase_cycles += NES_CPU_CLOCK_HZ / sample_rate_hz as f32;
    while channel.timer_phase_cycles >= timer_cycles {
        channel.timer_phase_cycles -= timer_cycles;
        channel.sequence_index = (channel.sequence_index + 1) & 0x07;
    }

    PULSE_DUTY_SEQUENCES[channel.duty_mode as usize][channel.sequence_index as usize] as f32
        * quantize_volume(channel.volume) as f32
}

fn render_triangle(channel: &mut TriangleChannel, sample_rate_hz: u32) -> f32 {
    if !channel.enabled || channel.volume <= 0.0 || channel.frequency_hz <= 0.0 {
        return 0.0;
    }

    let timer_cycles = 32.0 * (channel.timer_period as f32 + 1.0);
    channel.timer_phase_cycles += NES_CPU_CLOCK_HZ / sample_rate_hz as f32;
    while channel.timer_phase_cycles >= timer_cycles {
        channel.timer_phase_cycles -= timer_cycles;
        channel.sequence_index = (channel.sequence_index + 1) & 0x1f;
    }

    TRIANGLE_SEQUENCE[channel.sequence_index as usize] as f32 * channel.volume
}

fn render_noise(channel: &mut NoiseChannel, sample_rate_hz: u32) -> f32 {
    if !channel.enabled || channel.volume <= 0.0 {
        return 0.0;
    }

    let timer_cycles = NOISE_PERIOD_CYCLES[channel.period_index as usize] as f32;
    channel.timer_phase_cycles += NES_CPU_CLOCK_HZ / sample_rate_hz as f32;
    while channel.timer_phase_cycles >= timer_cycles {
        channel.timer_phase_cycles -= timer_cycles;
        let tap = if channel.short_mode { 6 } else { 1 };
        let feedback = ((channel.lfsr & 1) ^ ((channel.lfsr >> tap) & 1)) & 1;
        channel.lfsr = (channel.lfsr >> 1) | (feedback << 14);
        if channel.lfsr == 0 {
            channel.lfsr = 1;
        }
    }

    if channel.lfsr & 1 == 0 {
        quantize_volume(channel.volume) as f32
    } else {
        0.0
    }
}

fn render_dpcm(channel: &mut DpcmChannel, sample_rate_hz: u32) -> f32 {
    if channel.enabled {
        let timer_cycles = DPCM_PERIOD_CYCLES[channel.rate_index as usize] as f32;
        channel.timer_phase_cycles += NES_CPU_CLOCK_HZ / sample_rate_hz as f32;

        while channel.enabled && channel.timer_phase_cycles >= timer_cycles {
            channel.timer_phase_cycles -= timer_cycles;
            clock_dpcm_bit(channel);
        }
    }

    if channel.output_active {
        channel.output_level as f32
    } else {
        0.0
    }
}

fn clock_dpcm_bit(channel: &mut DpcmChannel) {
    if channel.bits_remaining == 0 {
        if channel.byte_index >= channel.sample.len() {
            if channel.looping {
                channel.byte_index = 0;
            } else {
                channel.enabled = false;
                return;
            }
        }

        channel.shift_register = channel.sample[channel.byte_index];
        channel.byte_index += 1;
        channel.bits_remaining = 8;
    }

    if channel.shift_register & 1 == 1 {
        if channel.output_level <= 125 {
            channel.output_level += 2;
        }
    } else if channel.output_level >= 2 {
        channel.output_level -= 2;
    }

    channel.shift_register >>= 1;
    channel.bits_remaining -= 1;
}

#[cfg(test)]
mod tests {
    use super::Apu;

    fn dpcm_signature(looping: bool) -> i64 {
        static SAMPLE: [u8; 4] = [0xff, 0x00, 0x96, 0x69];
        let mut apu = Apu::new(22_050);
        apu.start_dpcm(&SAMPLE, 15, looping);
        (0..4096)
            .map(|index| (index as i64 + 1) * apu.next_sample() as i64)
            .sum()
    }

    #[test]
    fn disabled_channels_are_silent() {
        let mut apu = Apu::new(22_050);
        assert!((0..128).all(|_| apu.next_sample() == 0));
    }

    #[test]
    fn channels_produce_mixed_audio() {
        let mut apu = Apu::new(22_050);
        apu.set_pulse(0, true, 440.0, 0.7, 0.125);
        apu.set_pulse(1, true, 660.0, 0.5, 0.5);
        apu.set_triangle(true, 220.0, 0.6);
        apu.set_noise(true, 4, 0.2);

        let mut minimum = i16::MAX;
        let mut maximum = i16::MIN;
        let mut transitions = 0;
        let mut previous = 0;
        for index in 0..4096 {
            let sample = apu.next_sample();
            minimum = minimum.min(sample);
            maximum = maximum.max(sample);
            if index > 0 && sample != previous {
                transitions += 1;
            }
            previous = sample;
        }

        assert!(transitions > 100);
        assert!(minimum < 0);
        assert!(maximum > 0);
    }

    #[test]
    fn noise_modes_differ() {
        fn signature(short_mode: bool) -> i64 {
            let mut apu = Apu::new(22_050);
            apu.set_noise(true, 4, 0.8);
            apu.set_noise_mode(short_mode);
            (0..1024)
                .map(|index| (index as i64 + 1) * apu.next_sample() as i64)
                .sum()
        }

        assert_ne!(signature(false), signature(true));
    }

    #[test]
    fn dpcm_looping_changes_the_output() {
        assert_ne!(dpcm_signature(false), dpcm_signature(true));
    }

    #[test]
    fn empty_dpcm_sample_stops_playback() {
        let mut apu = Apu::new(22_050);
        apu.start_dpcm(&[], 0, true);
        assert!((0..128).all(|_| apu.next_sample() == 0));
    }

    #[test]
    fn invalid_pulse_index_is_ignored() {
        let mut apu = Apu::new(22_050);
        apu.set_pulse(2, true, 440.0, 1.0, 0.5);
        assert!((0..128).all(|_| apu.next_sample() == 0));
    }
}
