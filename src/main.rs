#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
use embassy_executor::Spawner;
#[cfg(target_arch = "arm")]
use embassy_rp::pwm::{Config, Pwm, SetDutyCycle};
#[cfg(target_arch = "arm")]
use embassy_time::{Duration, Ticker};
#[cfg(target_arch = "arm")]
use nes_sound::apu::Apu;
#[cfg(target_arch = "arm")]
use panic_halt as _;

#[cfg(target_arch = "arm")]
const AUDIO_SAMPLE_RATE_HZ: u32 = 22_050;
#[cfg(target_arch = "arm")]
const TEMPO_TICKS_PER_SECOND: u32 = 60;
#[cfg(target_arch = "arm")]
const AUDIO_PWM_WRAP: u16 = 255;

#[cfg(target_arch = "arm")]
struct SongStep {
    duration_ticks: u16,
    pulse_hz: [f32; 2],
    triangle_hz: f32,
    pulse_volume: [f32; 2],
    triangle_volume: f32,
    pulse_duty: [f32; 2],
    noise_enabled: bool,
    noise_short_mode: bool,
    noise_period_index: u8,
    noise_volume: f32,
}

#[cfg(target_arch = "arm")]
const DEMO_SONG: [SongStep; 8] = [
    SongStep { duration_ticks: 12, pulse_hz: [523.25, 783.99], triangle_hz: 130.81, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.125, 0.25], noise_enabled: true, noise_short_mode: false, noise_period_index: 5, noise_volume: 0.20 },
    SongStep { duration_ticks: 12, pulse_hz: [659.25, 987.77], triangle_hz: 164.81, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.125, 0.25], noise_enabled: false, noise_short_mode: false, noise_period_index: 0, noise_volume: 0.00 },
    SongStep { duration_ticks: 12, pulse_hz: [783.99, 1174.66], triangle_hz: 196.00, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.250, 0.50], noise_enabled: true, noise_short_mode: true, noise_period_index: 4, noise_volume: 0.20 },
    SongStep { duration_ticks: 12, pulse_hz: [659.25, 987.77], triangle_hz: 164.81, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.125, 0.25], noise_enabled: false, noise_short_mode: false, noise_period_index: 0, noise_volume: 0.00 },
    SongStep { duration_ticks: 12, pulse_hz: [587.33, 880.00], triangle_hz: 146.83, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.125, 0.25], noise_enabled: true, noise_short_mode: false, noise_period_index: 6, noise_volume: 0.18 },
    SongStep { duration_ticks: 12, pulse_hz: [659.25, 987.77], triangle_hz: 164.81, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.250, 0.50], noise_enabled: false, noise_short_mode: false, noise_period_index: 0, noise_volume: 0.00 },
    SongStep { duration_ticks: 12, pulse_hz: [698.46, 1046.50], triangle_hz: 174.61, pulse_volume: [0.55, 0.35], triangle_volume: 0.35, pulse_duty: [0.125, 0.25], noise_enabled: true, noise_short_mode: true, noise_period_index: 5, noise_volume: 0.20 },
    SongStep { duration_ticks: 24, pulse_hz: [783.99, 1174.66], triangle_hz: 196.00, pulse_volume: [0.60, 0.40], triangle_volume: 0.40, pulse_duty: [0.250, 0.50], noise_enabled: true, noise_short_mode: false, noise_period_index: 3, noise_volume: 0.16 },
];

#[cfg(target_arch = "arm")]
static DPCM_DEMO_SAMPLE: [u8; 32] = [
    0x11, 0x33, 0x55, 0x77, 0x7f, 0x6f, 0x5f, 0x4f, 0x3f, 0x2f, 0x1f, 0x0f, 0x00, 0x24, 0x48,
    0x6c, 0x7e, 0x5a, 0x36, 0x12, 0x03, 0x27, 0x4b, 0x6f, 0x7f, 0x5f, 0x3f, 0x1f, 0x08, 0x2a,
    0x4c, 0x6e,
];

#[cfg(target_arch = "arm")]
fn apply_song_step(apu: &mut Apu, step: &SongStep) {
    apu.set_pulse(0, true, step.pulse_hz[0], step.pulse_volume[0], step.pulse_duty[0]);
    apu.set_pulse(1, true, step.pulse_hz[1], step.pulse_volume[1], step.pulse_duty[1]);
    apu.set_triangle(true, step.triangle_hz, step.triangle_volume);
    apu.set_noise(step.noise_enabled, step.noise_period_index, step.noise_volume);
    apu.set_noise_mode(step.noise_short_mode);
}

#[cfg(target_arch = "arm")]
#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());

    let mut pwm_config = Config::default();
    pwm_config.top = AUDIO_PWM_WRAP;
    pwm_config.compare_b = AUDIO_PWM_WRAP / 2;
    let mut pwm = Pwm::new_output_b(peripherals.PWM_SLICE0, peripherals.PIN_1, pwm_config);

    let mut apu = Apu::new(AUDIO_SAMPLE_RATE_HZ);
    apu.start_dpcm(&DPCM_DEMO_SAMPLE, 10, true);

    let mut step_index = 0;
    let mut ticks_until_step_change = DEMO_SONG[0].duration_ticks as u32;
    let mut samples_until_tick = AUDIO_SAMPLE_RATE_HZ / TEMPO_TICKS_PER_SECOND;
    apply_song_step(&mut apu, &DEMO_SONG[step_index]);

    let mut ticker = Ticker::every(Duration::from_micros(45));
    loop {
        samples_until_tick -= 1;
        if samples_until_tick == 0 {
            samples_until_tick = AUDIO_SAMPLE_RATE_HZ / TEMPO_TICKS_PER_SECOND;
            ticks_until_step_change -= 1;
            if ticks_until_step_change == 0 {
                step_index = (step_index + 1) % DEMO_SONG.len();
                ticks_until_step_change = DEMO_SONG[step_index].duration_ticks as u32;
                apply_song_step(&mut apu, &DEMO_SONG[step_index]);
            }
        }

        let sample = apu.next_sample();
        let pwm_level = ((sample as i32 + 32768) >> 8) as u16;
        let _ = pwm.set_duty_cycle(pwm_level);
        ticker.next().await;
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {}
