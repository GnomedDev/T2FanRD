use std::{io::Write, path::PathBuf};

use crate::{
    config::{FanConfig, SpeedCurve},
    error::{Error, Result},
};

#[derive(Debug)]
pub struct FanController {
    manual_path: PathBuf,
    output_path: PathBuf,
    config: FanConfig,

    min_speed: u32,
    max_speed: u32,
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
            .parse()
            .map_err(Error::MinSpeedParse)?;

        let max_speed = std::fs::read_to_string(join_suffix(path.clone(), "_max"))
            .map_err(Error::MaxSpeedRead)?
            .trim_end()
            .parse()
            .map_err(Error::MaxSpeedParse)?;

        let manual_path = join_suffix(path.clone(), "_manual");
        let output_path = join_suffix(path, "_output");

        let this = Self {
            manual_path,
            output_path,
            config,
            min_speed,
            max_speed,
        };

        println!("Found fan: {this:#?}");
        Ok(this)
    }

    pub fn set_manual(&self, enabled: bool) -> Result<()> {
        let res = std::fs::write(&self.manual_path, if enabled { "1" } else { "0" });
        res.map_err(Error::FanWrite)
    }

    pub fn set_speed(&self, mut speed: u32) -> Result<()> {
        if speed < self.min_speed {
            speed = self.min_speed;
        } else if speed > self.max_speed {
            speed = self.max_speed;
        }

        print!("\x1b[1K\rSetting fan speed to {speed}");
        let _ = std::io::stdout().lock().flush();
        std::fs::write(&self.output_path, speed.to_string()).map_err(Error::FanWrite)
    }

    pub fn calc_speed(&self, temp: u8) -> u32 {
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
        let low_temp = self.config.low_temp as u32;
        let high_temp = self.config.high_temp as u32;
        match self.config.speed_curve {
            SpeedCurve::Linear => {
                ((temp - low_temp) as f32 / (high_temp - low_temp) as f32
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
            SpeedCurve::Exponential => {
                ((temp - low_temp).pow(3) as f32 / (high_temp - low_temp).pow(3) as f32
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
            SpeedCurve::Logarithmic => {
                (((temp - low_temp) as f32).log((high_temp - low_temp) as f32)
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
        }
    }
}
