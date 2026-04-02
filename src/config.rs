use std::{fmt::Display, io::ErrorKind, num::NonZeroUsize, path::PathBuf, str::FromStr};

use nonempty::NonEmpty as NonEmptyVec;

use crate::{fan_controller::FanController, Error, Result};

#[cfg(debug_assertions)]
const CONFIG_FILE: &str = "./t2fand.conf";
#[cfg(not(debug_assertions))]
const CONFIG_FILE: &str = "/etc/t2fand.conf";

#[derive(Clone, Copy, Debug)]
pub enum SpeedCurve {
    Linear,
    Exponential,
    Logarithmic,
}

impl std::fmt::Display for SpeedCurve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linear => f.write_str("linear"),
            Self::Exponential => f.write_str("exponential"),
            Self::Logarithmic => f.write_str("logarithmic"),
        }
    }
}

impl FromStr for SpeedCurve {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "linear" => Self::Linear,
            "exponential" => Self::Exponential,
            "logarithmic" => Self::Logarithmic,
            _ => return Err(()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct FanConfig {
    pub low_temp: u8,
    pub high_temp: u8,
    pub speed_curve: SpeedCurve,
    pub always_full_speed: bool,
    pub sensor_group: SensorGroup,
}

#[derive(Clone, Debug)]
pub enum SensorGroup {
    Average(Vec<TempSensor>),
    Max(Vec<TempSensor>),
    One(TempSensor),
}

#[derive(Clone, Copy, Debug)]
pub enum TempSensor {
    CPU,
    GPU,
}

impl Display for TempSensor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TempSensor::CPU => f.write_str("CPU"),
            TempSensor::GPU => f.write_str("GPU"),
        }
    }
}
impl FromStr for SensorGroup {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle variants using square brackets: Max[...] and Average[...]
        if let Some((variant_str, content)) = s.split_once('[') {
            let sensors_str = content.strip_suffix(']').ok_or(())?;

            let sensors: Vec<TempSensor> = sensors_str
                .split(',')
                .filter_map(|item| match item.trim() {
                    "CPU" => Some(TempSensor::CPU),
                    "GPU" => Some(TempSensor::GPU),
                    _ => None,
                })
                .collect();

            match variant_str.trim() {
                "Max" => Ok(SensorGroup::Max(sensors)),
                "Average" => Ok(SensorGroup::Average(sensors)),
                _ => Err(()),
            }
        }
        // Handle the variant using parentheses: One(...)
        else if let Some((variant_str, content)) = s.split_once('(') {
            let sensor_str = content.strip_suffix(')').ok_or(())?;

            let sensor = match sensor_str.trim() {
                "CPU" => TempSensor::CPU,
                "GPU" => TempSensor::GPU,
                _ => return Err(()),
            };

            match variant_str.trim() {
                "One" => Ok(SensorGroup::One(sensor)),
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}

impl std::fmt::Display for SensorGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensorGroup::Average(temp_sensors) => write!(f, "Average{:?}", temp_sensors),
            SensorGroup::Max(temp_sensors) => write!(f, "Max{:?}", temp_sensors),
            SensorGroup::One(temp_sensor) => write!(f, "One({:?})", temp_sensor),
        }
    }
}

impl FanConfig {
    fn write_property<'a>(
        self,
        setter: &'a mut ini::SectionSetter<'a>,
    ) -> &'a mut ini::SectionSetter<'a> {
        setter
            .set("low_temp", self.low_temp.to_string())
            .set("high_temp", self.high_temp.to_string())
            .set("speed_curve", self.speed_curve.to_string())
            .set("always_full_speed", self.always_full_speed.to_string())
            .set("sensor_group", self.sensor_group.to_string())
    }
}

impl Default for FanConfig {
    fn default() -> Self {
        Self {
            low_temp: 55,
            high_temp: 75,
            speed_curve: SpeedCurve::Linear,
            always_full_speed: false,
            sensor_group: SensorGroup::Max(vec![TempSensor::CPU, TempSensor::GPU]),
        }
    }
}

impl TryFrom<&ini::Properties> for FanConfig {
    type Error = Error;

    fn try_from(properties: &ini::Properties) -> Result<Self, Self::Error> {
        fn get_value<V: FromStr>(properties: &ini::Properties, key: &'static str) -> Result<V> {
            let value_str = properties.get(key).ok_or(Error::MissingConfigValue(key))?;
            value_str
                .parse()
                .map_err(|_| Error::InvalidConfigValue(key))
        }

        Ok(Self {
            low_temp: get_value(properties, "low_temp")?,
            high_temp: get_value(properties, "high_temp")?,
            speed_curve: get_value(properties, "speed_curve")?,
            always_full_speed: get_value(properties, "always_full_speed")?,
            sensor_group: get_value(properties, "sensor_group")?,
        })
    }
}

fn parse_config_file(file_raw: &str, fan_count: NonZeroUsize) -> Result<Vec<FanConfig>> {
    let file = ini::Ini::load_from_str(file_raw)?;
    let mut configs = Vec::with_capacity(fan_count.get());

    for i in 1..=fan_count.get() {
        let section = file
            .section(Some(format!("Fan{i}")))
            .ok_or(Error::MissingFanConfig(i))?;

        configs.push(FanConfig::try_from(section)?);
    }

    Ok(configs)
}

fn generate_config_file(fan_count: NonZeroUsize) -> Result<Vec<FanConfig>> {
    let mut config_file = ini::Ini::new();
    let mut configs = Vec::with_capacity(fan_count.get());
    for i in 1..=fan_count.get() {
        let config = FanConfig::default();
        configs.push(config.clone());

        let mut setter = config_file.with_section(Some(format!("Fan{i}")));
        config.write_property(&mut setter);
    }

    config_file
        .write_to_file(CONFIG_FILE)
        .map_err(Error::ConfigCreate)?;

    Ok(configs)
}

pub fn load_fan_configs(fan_paths: NonEmptyVec<PathBuf>) -> Result<NonEmptyVec<FanController>> {
    let fan_count = fan_paths.len_nonzero();
    let configs = match std::fs::read_to_string(CONFIG_FILE) {
        Ok(file_raw) => parse_config_file(&file_raw, fan_count)?,
        Err(err) if err.kind() == ErrorKind::NotFound => generate_config_file(fan_count)?,
        Err(err) => return Err(Error::ConfigRead(err)),
    };

    let fans = fan_paths
        .into_iter()
        .zip(configs)
        .map(|(path, config)| FanController::new(path, config))
        .collect::<Result<_>>()?;

    Ok(NonEmptyVec::from_vec(fans).unwrap())
}
