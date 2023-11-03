#![warn(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::module_name_repetitions
)]

use std::{
    collections::VecDeque,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use config::load_fan_configs;
use nonempty::NonEmpty as NonEmptyVec;

use error::{Error, Result};
use signal_hook::consts::{SIGINT, SIGTERM};

mod config;
mod error;
mod fan_controller;

#[cfg(not(target_os = "linux"))]
compile_error!("This tool is only developed for Linux systems.");

#[cfg(debug_assertions)]
const PID_FILE: &str = "t2fand.pid";
#[cfg(not(debug_assertions))]
const PID_FILE: &str = "/run/t2fand.pid";

fn get_current_euid() -> libc::uid_t {
    // SAFETY: FFI call with no preconditions
    unsafe { libc::geteuid() }
}

fn find_fan_paths() -> Result<NonEmptyVec<PathBuf>> {
    // APP0001:00/fan1_label
    let fan = glob::glob("/sys/devices/pci*/*/*/*/APP0001:00/fan*")?
        .filter_map(Result::ok)
        .find(|p| p.exists())
        .ok_or(Error::NoFan)?;

    // APP0001:00
    let first_fan_path = fan.parent().ok_or(Error::NoFan)?;
    // APP0001:00/fan*_input
    let fan_glob = first_fan_path.display().to_string() + "/fan*_input";
    // APP0001:00/fan1
    let fans = glob::glob(&fan_glob)?
        .filter_map(Result::ok)
        .filter_map(|mut path| {
            let file_name = path.file_name()?.to_str()?;
            let fan_name = file_name.strip_suffix("_input")?;
            let fan_name_owned = fan_name.to_owned();
            path.set_file_name(fan_name_owned);
            Some(path)
        });

    NonEmptyVec::collect(fans).ok_or(Error::NoFan)
}

fn check_pid_file() -> Result<()> {
    match std::fs::read_to_string(PID_FILE) {
        Ok(pid) => {
            let mut proc_path = std::path::PathBuf::new();
            proc_path.push("/proc");
            proc_path.push(pid);

            if proc_path.exists() {
                return Err(Error::AlreadyRunning);
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(Error::PidRead(err)),
    };

    let current_pid = std::process::id().to_string();
    std::fs::write(PID_FILE, current_pid).map_err(Error::PidWrite)
}

fn read_temp_file(temp_path: &Path, temp_buf: &mut String) -> Result<u8> {
    let mut temp_file = std::fs::File::open(temp_path).map_err(Error::TempOpen)?;

    temp_file
        .read_to_string(temp_buf)
        .map_err(Error::TempRead)?;

    let temp = temp_buf.trim_end().parse::<u32>().map_err(Error::TempParse);
    temp_buf.clear();
    temp.map(|t| (t / 1000) as u8)
}

fn find_temp_file(temps: glob::Paths, temp_buf: &mut String) -> Option<PathBuf> {
    for temp_path_res in temps {
        let Ok(temp_path) = temp_path_res else {
            eprintln!("Unable to read glob path");
            continue;
        };

        if read_temp_file(&temp_path, temp_buf).is_ok() {
            return Some(temp_path);
        }
    }

    None
}

fn find_cpu_temp_file(temp_buf: &mut String) -> Result<PathBuf> {
    let temps = glob::glob("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input")?;
    find_temp_file(temps, temp_buf).ok_or(Error::NoCpu)
}

fn find_gpu_temp_file(temp_buf: &mut String) -> Result<Option<PathBuf>> {
    let temps = glob::glob("/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input")?;
    Ok(find_temp_file(temps, temp_buf))
}

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn real_main() -> Result<()> {
    if get_current_euid() != 0 {
        return Err(Error::NotRoot);
    }

    check_pid_file()?;

    let mut temp_buffer = String::new();

    let fan_paths = find_fan_paths()?;
    let fans = load_fan_configs(fan_paths)?;
    let cpu_temp_path = find_cpu_temp_file(&mut temp_buffer)?;
    let gpu_temp_path = find_gpu_temp_file(&mut temp_buffer)?;

    println!();
    for fan in &fans {
        fan.set_manual(true)?;
    }

    let cancellation_token = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, cancellation_token.clone()).map_err(Error::Signal)?;
    signal_hook::flag::register(SIGTERM, cancellation_token.clone()).map_err(Error::Signal)?;

    let mut temps = VecDeque::with_capacity(5);
    while !cancellation_token.load(std::sync::atomic::Ordering::Relaxed) {
        let cpu_temp = read_temp_file(&cpu_temp_path, &mut temp_buffer)?;
        let temp = if let Some(gpu_temp_path) = &gpu_temp_path {
            let gpu_temp = read_temp_file(gpu_temp_path, &mut temp_buffer)?;
            if gpu_temp > cpu_temp {
                gpu_temp
            } else {
                cpu_temp
            }
        } else {
            cpu_temp
        };

        if temps.len() > 5 {
            temps.pop_front();
        }

        temps.push_back(temp);

        let sum_temp: u16 = temps.iter().map(|t| *t as u16).sum();
        let mean_temp = sum_temp / (temps.len() as u16);
        for fan in &fans {
            fan.set_speed(fan.calc_speed(mean_temp as u8))?;
        }

        std::thread::sleep(Duration::new(1, 0));
    }

    println!("T2 Fan Daemon is shutting down...");
    for fan in fans {
        fan.set_manual(false)?;
    }

    std::fs::remove_file(PID_FILE).map_err(Error::PidDelete)
}
