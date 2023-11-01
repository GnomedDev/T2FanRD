use std::path::PathBuf;

use crate::{
    config::{FanConfig, SpeedCurve},
    error::{Error, Result},
};

pub struct FanController {
    manual_path: PathBuf,
    output_path: PathBuf,
    config: FanConfig,

    min_speed: u8,
    max_speed: u8,
}

impl FanController {
    pub fn new(path: PathBuf, config: FanConfig) -> Result<Self> {
        let min_speed = std::fs::read_to_string(path.join("_min"))
            .map_err(Error::MinSpeedRead)?
            .parse()
            .map_err(Error::MinSpeedParse)?;

        let max_speed = std::fs::read_to_string(path.join("_max"))
            .map_err(Error::MaxSpeedRead)?
            .parse()
            .map_err(Error::MaxSpeedParse)?;

        let manual_path = path.join("_manual");

        let mut output_path = path;
        output_path.push("_output");

        Ok(Self {
            manual_path,
            output_path,
            config,
            min_speed,
            max_speed,
        })
    }

    pub fn set_manual(&self, enabled: bool) -> Result<()> {
        let res = std::fs::write(&self.manual_path, if enabled { "1" } else { "0" });
        res.map_err(Error::FanWrite)
    }

    pub fn set_speed(&self, mut speed: u8) -> Result<()> {
        if speed < self.min_speed {
            speed = self.min_speed;
        } else if speed > self.max_speed {
            speed = self.max_speed;
        }

        std::fs::write(&self.output_path, speed.to_string()).map_err(Error::FanWrite)
    }

    pub fn calc_speed(&self, temp: u8) -> u8 {
        if self.config.always_full_speed {
            return self.max_speed;
        }

        if temp <= self.config.low_temp {
            return self.min_speed;
        }
        if temp >= self.config.high_temp {
            return self.max_speed;
        }

        match self.config.speed_curve {
            SpeedCurve::Linear => {
                (temp - self.config.low_temp) / (self.config.high_temp - self.config.low_temp)
                    * (self.max_speed - self.min_speed)
                    + self.min_speed
            }

            SpeedCurve::Exponential => {
                (temp - self.config.low_temp).pow(3)
                    / (self.config.high_temp - self.config.low_temp).pow(3)
                    * (self.max_speed - self.min_speed)
                    + self.min_speed
            }

            SpeedCurve::Logarithmic => {
                ((temp - self.config.low_temp) as f32)
                    .log((self.config.high_temp - self.config.low_temp) as f32)
                    as u8
                    * (self.max_speed - self.min_speed)
                    + self.min_speed
            }
        }
    }
}
