use std::{io::Write, num::NonZeroU16, path::PathBuf};

use crate::{
    config::{FanConfig, SpeedCurve},
    error::{Error, Result},
};

fn clamp_minimum(value: u16, minimum: Option<NonZeroU16>) -> u16 {
    value.max(minimum.map_or(u16::MIN, NonZeroU16::get))
}

fn clamp_maximum(value: u16, maximum: Option<NonZeroU16>) -> u16 {
    value.min(maximum.map_or(u16::MAX, NonZeroU16::get))
}

#[derive(Debug)]
pub struct FanController {
    manual_file: std::fs::File,
    output_file: std::fs::File,
    config: FanConfig,

    min_speed: u16,
    max_speed: u16,
}

impl FanController {
    pub fn new(path: PathBuf, config: FanConfig) -> Result<Self> {
        fn join_suffix(mut path: PathBuf, suffix: &str) -> PathBuf {
            let file_name = path.file_name().unwrap().to_str().unwrap();
            path.set_file_name(format!("{file_name}{suffix}"));
            path
        }

        let min_speed = std::fs::read_to_string(join_suffix(path.clone(), "_min"))
            .map_err(Error::MinSpeedRead)?
            .trim()
            .parse::<u16>()
            .map(|value| clamp_minimum(value, config.min_speed_override))
            .map_err(Error::MinSpeedParse)?;

        let max_speed = std::fs::read_to_string(join_suffix(path.clone(), "_max"))
            .map_err(Error::MaxSpeedRead)?
            .trim_end()
            .parse::<u16>()
            .map(|value| clamp_maximum(value, config.max_speed_override))
            .map_err(Error::MaxSpeedParse)?;

        let mut open_options = std::fs::OpenOptions::new();
        open_options.write(true).truncate(true);

        let manual_file = open_options
            .open(join_suffix(path.clone(), "_manual"))
            .map_err(Error::FanOpen)?;

        let output_file = open_options
            .open(join_suffix(path, "_output"))
            .map_err(Error::FanOpen)?;

        let this = Self {
            manual_file,
            output_file,
            config,
            min_speed,
            max_speed,
        };

        println!("Found fan: {this:#?}");
        Ok(this)
    }

    #[cfg(test)]
    pub fn new_dummy(min_speed: u16, max_speed: u16, config: FanConfig) -> Self {
        Self {
            config,
            min_speed: clamp_minimum(min_speed, config.min_speed_override),
            max_speed: clamp_maximum(max_speed, config.max_speed_override),
            manual_file: std::fs::File::open("/dev/null").unwrap(),
            output_file: std::fs::File::open("/dev/null").unwrap(),
        }
    }

    pub fn set_manual(&self, enabled: bool) -> Result<()> {
        (&self.manual_file)
            .write_all(if enabled { b"1" } else { b"0" })
            .map_err(Error::FanWrite)
    }

    pub fn set_speed(&self, mut speed: u16) -> Result<()> {
        if speed < self.min_speed {
            speed = self.min_speed;
        } else if speed > self.max_speed {
            speed = self.max_speed;
        }

        print!("\x1b[1K\rSetting fan speed to {speed}");
        let _ = std::io::stdout().lock().flush();

        write!(&self.output_file, "{speed}").map_err(Error::FanWrite)?;
        Ok(())
    }

    pub fn calc_speed(&self, temp: u8) -> u16 {
        if self.config.always_full_speed {
            return self.max_speed;
        }

        if temp <= self.config.low_temp {
            return self.min_speed;
        }
        if temp >= self.config.high_temp {
            return self.max_speed;
        }

        let temp = temp as u32;
        let min_speed = self.min_speed as u16;
        let max_speed = self.max_speed as u16;
        let low_temp = self.config.low_temp as u32;
        let high_temp = self.config.high_temp as u32;
        match self.config.speed_curve {
            SpeedCurve::Linear => {
                (((temp - low_temp) as f32 / (high_temp - low_temp) as f32
                    * (max_speed - min_speed) as f32) as u16
                    + min_speed) as u16
            }
            SpeedCurve::Exponential => {
                (((temp - low_temp).pow(3) as f32 / (high_temp - low_temp).pow(3) as f32
                    * (max_speed - min_speed) as f32) as u16
                    + min_speed) as u16
            }
            SpeedCurve::Logarithmic => {
                ((((temp - low_temp) as f32).log((high_temp - low_temp) as f32)
                    * (max_speed - min_speed) as f32) as u16
                    + min_speed) as u16
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use crate::config::{FanConfig, SpeedCurve};

    use super::FanController;

    const EXPECTED_LINEAR_CURVE: [u16; 110] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 200, 300, 400, 500,
        600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600, 1700, 1800, 1900, 2000, 2100,
        2200, 2300, 2400, 2500, 2600, 2700, 2800, 2900, 3000, 3100, 3200, 3300, 3400, 3500, 3600,
        3700, 3800, 3899, 4000, 4100, 4200, 4300, 4400, 4500, 4600, 4700, 4800, 4900, 5000, 5000,
        5000, 5000, 5000, 5000, 5000, 5000, 5000, 5000,
    ];

    const EXPECTED_EXPONENTIAL_CURVE: [u16; 110] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 5, 8, 13, 20,
        29, 40, 53, 69, 87, 109, 135, 163, 196, 233, 274, 320, 370, 425, 486, 552, 625, 703, 787,
        878, 975, 1080, 1191, 1310, 1437, 1572, 1715, 1866, 2026, 2194, 2372, 2560, 2756, 2963,
        3180, 3407, 3644, 3893, 4152, 4423, 4705, 5000, 5000, 5000, 5000, 5000, 5000, 5000, 5000,
        5000, 5000,
    ];

    const EXPECTED_LOGARITHMIC_CURVE: [u16; 110] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 885, 1404, 1771, 2057,
        2290, 2487, 2657, 2808, 2942, 3064, 3175, 3278, 3373, 3461, 3543, 3621, 3694, 3763, 3828,
        3891, 3950, 4007, 4061, 4114, 4164, 4212, 4258, 4303, 4347, 4389, 4429, 4468, 4507, 4544,
        4580, 4615, 4649, 4682, 4714, 4746, 4777, 4807, 4836, 4865, 4893, 4920, 4947, 4974, 5000,
        5000, 5000, 5000, 5000, 5000, 5000, 5000, 5000, 5000,
    ];

    fn calculate(
        speed_curve: SpeedCurve,
        min_speed_override: Option<NonZeroU16>,
        max_speed_override: Option<NonZeroU16>,
    ) -> [u16; 110] {
        let config = FanConfig {
            speed_curve,
            low_temp: 50,
            high_temp: 100,
            min_speed_override,
            max_speed_override,
            always_full_speed: false,
        };

        let controller = FanController::new_dummy(0, 5000, config);
        std::array::from_fn(|temp| controller.calc_speed(temp.try_into().unwrap()))
    }

    #[test]
    fn test_calc_speed() {
        assert_eq!(
            calculate(SpeedCurve::Linear, None, None),
            EXPECTED_LINEAR_CURVE
        );
        assert_eq!(
            calculate(SpeedCurve::Exponential, None, None),
            EXPECTED_EXPONENTIAL_CURVE
        );
        assert_eq!(
            calculate(SpeedCurve::Logarithmic, None, None),
            EXPECTED_LOGARITHMIC_CURVE
        );
    }

    // Ramps up from the minimum value faster, instead of just simply starting the ramp later.
    const EXPECTED_LINEAR_MIN: [u16; 110] = [
        1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
        1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
        1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
        1000, 1000, 1000, 1000, 1000, 1000, 1080, 1160, 1240, 1320, 1400, 1480, 1560, 1640, 1720,
        1800, 1880, 1960, 2040, 2120, 2200, 2280, 2360, 2440, 2520, 2600, 2680, 2760, 2840, 2920,
        3000, 3080, 3160, 3240, 3320, 3400, 3480, 3560, 3640, 3720, 3800, 3880, 3960, 4040, 4120,
        4200, 4280, 4360, 4440, 4520, 4600, 4680, 4760, 4840, 4920, 5000, 5000, 5000, 5000, 5000,
        5000, 5000, 5000, 5000, 5000,
    ];

    #[test]
    fn test_calc_speed_with_min() {
        assert_eq!(
            calculate(SpeedCurve::Linear, NonZeroU16::new(1000), None),
            EXPECTED_LINEAR_MIN
        );
    }

    // Ramps up to the maximum value slower, instead of capping at the top speed for longer.
    const EXPECTED_LINEAR_MAX: [u16; 110] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 80, 160, 240, 320, 400, 480,
        560, 640, 720, 800, 880, 960, 1040, 1120, 1200, 1280, 1360, 1440, 1520, 1600, 1680, 1760,
        1840, 1920, 2000, 2080, 2160, 2240, 2320, 2400, 2480, 2560, 2640, 2720, 2800, 2880, 2960,
        3040, 3120, 3200, 3280, 3360, 3440, 3520, 3600, 3680, 3760, 3840, 3920, 4000, 4000, 4000,
        4000, 4000, 4000, 4000, 4000, 4000, 4000,
    ];

    #[test]
    fn test_calc_speed_with_max() {
        let max_override = NonZeroU16::new(4000);
        assert_eq!(
            calculate(SpeedCurve::Linear, None, max_override),
            EXPECTED_LINEAR_MAX
        );
    }
}
