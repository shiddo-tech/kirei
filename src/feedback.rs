use crate::settings::{self, CaptureSound};

const RATE: f64 = 44_100.;

/// Kirei's own sounds, synthesized as WAV data so they need no resource files.
fn synth(duration: f64, mut f: impl FnMut(f64) -> f64) -> Vec<i16> {
    let n = (duration * RATE) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE;
            let fade = ((n - i) as f64 / (RATE * 0.01)).min(1.);
            ((f(t) * fade).clamp(-1., 1.) * i16::MAX as f64) as i16
        })
        .collect()
}

pub fn wav(samples: &[i16]) -> Vec<u8> {
    let bytes = (samples.len() * 2) as u32;
    let mut d = Vec::with_capacity(44 + bytes as usize);
    d.extend_from_slice(b"RIFF");
    d.extend_from_slice(&(36 + bytes).to_le_bytes());
    d.extend_from_slice(b"WAVEfmt ");
    d.extend_from_slice(&16u32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&(RATE as u32).to_le_bytes());
    d.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
    d.extend_from_slice(&2u16.to_le_bytes());
    d.extend_from_slice(&16u16.to_le_bytes());
    d.extend_from_slice(b"data");
    d.extend_from_slice(&bytes.to_le_bytes());
    for s in samples {
        d.extend_from_slice(&s.to_le_bytes());
    }
    d
}

/// A soft two-part shutter: a filtered noise click followed by a lighter one.
fn shutter() -> Vec<u8> {
    let mut rng = crate::render::Rng::from_time();
    let mut lp = 0.;
    let samples = synth(0.16, |t| {
        let n = (rng.next() % 20001) as f64 / 10000. - 1.;
        lp += (n - lp) * 0.35;
        let first = (-t * 140.).exp() * 0.55;
        let second = if t > 0.065 {
            (-(t - 0.065) * 170.).exp() * 0.32
        } else {
            0.
        };
        let body = (2. * std::f64::consts::PI * 1_900. * t).sin() * (-t * 90.).exp() * 0.12;
        lp * (first + second) + body
    });
    wav(&samples)
}

/// Two short glassy notes, a fifth apart.
fn copy() -> Vec<u8> {
    let note = |t: f64, f: f64, start: f64| {
        if t < start {
            return 0.;
        }
        let u = t - start;
        let attack = (u / 0.004).min(1.);
        let tau = 2. * std::f64::consts::PI;
        ((tau * f * u).sin() + 0.25 * (tau * f * 2. * u).sin()) * attack * (-u * 18.).exp()
    };
    wav(&synth(0.32, |t| {
        0.32 * (note(t, 1_318.5, 0.) + note(t, 1_975.5, 0.07))
    }))
}

fn tick() -> Vec<u8> {
    wav(&synth(0.05, |t| {
        (2. * std::f64::consts::PI * 2_400. * t).sin() * (-t * 120.).exp() * 0.5
    }))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Sound {
    Voice,
    Shutter,
    Copy,
    Tick,
}

fn play(sound: Sound) {
    if !settings::get().play_sound {
        return;
    }
    #[cfg(target_os = "macos")]
    crate::platform::play_sound(sound as u8, || match sound {
        Sound::Voice => (include_bytes!("../Resources/kirei-voice.wav").to_vec(), 0.4),
        Sound::Shutter => (shutter(), 0.55),
        Sound::Copy => (copy(), 0.45),
        Sound::Tick => (tick(), 0.35),
    });
}

pub fn play_capture() {
    play(match settings::get().capture_sound {
        CaptureSound::Voice => Sound::Voice,
        CaptureSound::Shutter => Sound::Shutter,
    });
}

pub fn play_copy() {
    play(Sound::Copy);
}

pub fn play_tick() {
    play(Sound::Tick);
}
