//! The audio filter chain for own playback: preamp, 10-band EQ and balance.
//!
//! The chain is one FFmpeg `lavfi` graph in mpv. It changes only the engine's
//! own mpv output, never another stream on the desktop.
use crate::protocol::{BANDS, EqSettings, Failure};

/// Winamp 2's band centres in hertz.
pub const BAND_HZ: [u32; BANDS] = [
    60, 170, 310, 600, 1_000, 3_000, 6_000, 12_000, 14_000, 16_000,
];

const DB_LIMIT: f64 = 12.0;

pub fn validate_eq(eq: &EqSettings) -> Result<(), Failure> {
    let ok = eq.bands_db.len() == BANDS
        && std::iter::once(eq.preamp_db)
            .chain(eq.bands_db.iter().copied())
            .all(|db| db.is_finite() && (-DB_LIMIT..=DB_LIMIT).contains(&db));
    if ok {
        Ok(())
    } else {
        Err(Failure::new(
            "invalid",
            "EQ needs 10 bands and preamp in -12..12 dB",
        ))
    }
}

pub fn validate_balance(value: f64) -> Result<(), Failure> {
    if value.is_finite() && (-1.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Failure::new("invalid", "balance must be in -1..1"))
    }
}

fn flat(eq: &EqSettings) -> bool {
    !eq.on || (eq.preamp_db == 0.0 && eq.bands_db.iter().all(|db| *db == 0.0))
}

/// The value for mpv's `af` property. An empty string removes every filter.
///
/// The EQ uses `firequalizer`, which interpolates gain in dB between the band
/// centres. Unlike peaking biquads it stays valid when a band lies above the
/// stream's Nyquist frequency (for example 16 kHz in a 22.05 kHz file).
/// A balance other than 0 converts the stream to stereo first, so a
/// multichannel source is downmixed while balance is off centre.
pub fn graph(eq: &EqSettings, balance: f64) -> String {
    let mut parts = vec![];
    if !flat(eq) {
        if eq.preamp_db != 0.0 {
            parts.push(format!("volume={:.2}dB", eq.preamp_db));
        }
        if eq.bands_db.iter().any(|db| *db != 0.0) {
            let entries = BAND_HZ
                .iter()
                .zip(&eq.bands_db)
                .map(|(hz, db)| format!("entry({hz},{db:.2})"))
                .collect::<Vec<_>>()
                .join(";");
            parts.push(format!("firequalizer=delay=0.05:gain_entry='{entries}'"));
        }
    }
    if balance != 0.0 {
        let left = (1.0 - balance).min(1.0);
        let right = (1.0 + balance).min(1.0);
        parts.push(format!(
            "aformat=channel_layouts=stereo,pan=stereo|c0={left:.4}*c0|c1={right:.4}*c1"
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("@pyxamp:lavfi=[{}]", parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(on: bool, preamp: f64, bands: [f64; 10]) -> EqSettings {
        EqSettings {
            on,
            preamp_db: preamp,
            bands_db: bands.to_vec(),
        }
    }

    #[test]
    fn flat_or_off_eq_and_centre_balance_remove_every_filter() {
        assert_eq!(graph(&eq(true, 0.0, [0.0; 10]), 0.0), "");
        assert_eq!(graph(&eq(false, 6.0, [12.0; 10]), 0.0), "");
    }

    #[test]
    fn graph_names_each_band_preamp_and_balance() {
        let mut bands = [0.0; 10];
        bands[4] = 12.0;
        let text = graph(&eq(true, -3.0, bands), -0.5);
        assert!(text.starts_with("@pyxamp:lavfi=["), "{text}");
        assert!(text.contains("volume=-3.00dB"));
        assert!(text.contains("entry(1000,12.00)"));
        assert!(text.contains("entry(60,0.00)"));
        assert!(text.contains("c0=1.0000*c0|c1=0.5000*c1"));
        // mpv's list parser must see one bracketed lavfi graph and no nested brackets.
        assert_eq!(text.matches('[').count(), 1);
    }

    #[test]
    fn validation_rejects_out_of_range_values() {
        assert!(validate_eq(&eq(true, 0.0, [0.0; 10])).is_ok());
        assert!(validate_eq(&eq(true, 12.5, [0.0; 10])).is_err());
        assert!(validate_eq(&eq(true, f64::NAN, [0.0; 10])).is_err());
        let mut short = eq(true, 0.0, [0.0; 10]);
        short.bands_db.pop();
        assert!(validate_eq(&short).is_err());
        assert!(validate_balance(1.0).is_ok());
        assert!(validate_balance(-1.01).is_err());
    }
}
