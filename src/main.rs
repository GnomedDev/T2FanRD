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
    io::{ErrorKind, Read, Seek},
    path::PathBuf,
    process::ExitCode,
    sync::{atomic::AtomicBool, Arc},
};

use arraydeque::ArrayDeque;
use fan_controller::FanController;
use nonempty::NonEmpty as NonEmptyVec;
use signal_hook::consts::{SIGINT, SIGTERM};

use config::load_fan_configs;
use error::{Error, Result};

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
    }

    let current_pid = std::process::id().to_string();
    std::fs::write(PID_FILE, current_pid).map_err(Error::PidWrite)
}

fn read_temp_file(temp_file: &mut std::fs::File, temp_buf: &mut String) -> Result<u8> {
    temp_file
        .read_to_string(temp_buf)
        .map_err(Error::TempRead)?;

    temp_file.rewind().map_err(Error::TempSeek)?;

    let temp = temp_buf.trim_end().parse::<u32>().map_err(Error::TempParse);
    temp_buf.clear();
    temp.map(|t| (t / 1000) as u8)
}

fn find_temp_file(
    temps: impl Iterator<Item = glob::GlobResult>,
    temp_buf: &mut String,
) -> Option<std::fs::File> {
    for temp_path_res in temps {
        let Ok(temp_path) = temp_path_res else {
            eprintln!("Unable to read glob path");
            continue;
        };

        let Ok(mut temp_file) = std::fs::File::open(temp_path) else {
            eprintln!("Unable to open temperature sensor");
            continue;
        };

        if read_temp_file(&mut temp_file, temp_buf).is_ok() {
            return Some(temp_file);
        }
    }

    None
}

fn find_cpu_temp_file(temp_buf: &mut String) -> Result<std::fs::File> {
    let temps = glob::glob("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input")?;
    find_temp_file(temps, temp_buf).ok_or(Error::NoCpu)
}

fn is_amdgpu_sensor(temp_path: &std::path::Path) -> bool {
    temp_path
        .parent()
        .and_then(|hwmon| std::fs::read_to_string(hwmon.join("name")).ok())
        .is_some_and(|name| name.trim() == "amdgpu")
}

fn find_gpu_temp_file(temp_buf: &mut String) -> Result<Option<std::fs::File>> {
    // The discrete GPU is not always card0: the number depends on probe order,
    // and on a MacBookPro16,1 the Intel iGPU takes card1 and the AMD dGPU card2.
    // Look at every card and prefer the amdgpu sensor over anything else.
    let mut temps: Vec<PathBuf> =
        glob::glob("/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input")?
            .filter_map(Result::ok)
            .collect();
    temps.sort_by_key(|path| !is_amdgpu_sensor(path));
    Ok(find_temp_file(temps.into_iter().map(Ok), temp_buf))
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

fn start_temp_loop(
    mut temp_buffer: String,
    mut cpu_temp_file: std::fs::File,
    mut gpu_temp_file: Option<std::fs::File>,
    fans: &NonEmptyVec<FanController>,
) -> Result<()> {
    let cancellation_token = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, cancellation_token.clone()).map_err(Error::Signal)?;
    signal_hook::flag::register(SIGTERM, cancellation_token.clone()).map_err(Error::Signal)?;

    let mut last_temp = 0;
    let mut temps = ArrayDeque::<u8, 50, arraydeque::Wrapping>::new();
    let mut was_long_sleep = false;
    while !cancellation_token.load(std::sync::atomic::Ordering::Relaxed) {
        let cpu_temp = read_temp_file(&mut cpu_temp_file, &mut temp_buffer)?;
        let mut temp = cpu_temp;
        if let Some(file) = &mut gpu_temp_file {
            // A discrete GPU can disappear from under us: a reset, a runtime
            // suspend or a Thunderbolt re-enumeration all invalidate the open
            // sensor. Losing the GPU reading must not take the daemon down,
            // because it leaves the fans pinned at whatever speed it wrote
            // last while the machine keeps heating. Fall back to the CPU
            // sensor and stop looking at the GPU.
            match read_temp_file(file, &mut temp_buffer) {
                Ok(gpu_temp) => temp = temp.max(gpu_temp),
                Err(err) => {
                    eprintln!("GPU temperature unreadable ({err}), using the CPU sensor alone");
                    gpu_temp_file = None;
                }
            }
        }

        temps.push_back(temp);
        if was_long_sleep {
            // Avoid messing up the mean due to the longer sleep.
            for _ in 0..9 {
                temps.push_back(temp);
            }
        }

        let sum_temp: u16 = temps.iter().map(|t| *t as u16).sum();
        let mean_temp = sum_temp / (temps.len() as u16);
        if mean_temp == last_temp {
            std::thread::sleep(std::time::Duration::from_secs(1));
            was_long_sleep = true;
        } else {
            last_temp = mean_temp;
            for fan in fans {
                fan.set_speed(fan.calc_speed(mean_temp as u8))?;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
            was_long_sleep = false;
        }
    }

    Ok(())
}

fn real_main() -> Result<()> {
    if get_current_euid() != 0 {
        return Err(Error::NotRoot);
    }

    check_pid_file()?;

    let mut temp_buffer = String::new();

    let fan_paths = find_fan_paths()?;
    let fans = load_fan_configs(fan_paths)?;
    let cpu_temp_file = find_cpu_temp_file(&mut temp_buffer)?;
    let gpu_temp_file = find_gpu_temp_file(&mut temp_buffer)?;

    println!();
    for fan in &fans {
        fan.set_manual(true)?;
    }

    let res = start_temp_loop(temp_buffer, cpu_temp_file, gpu_temp_file, &fans);
    println!("T2 Fan Daemon is shutting down...");
    for fan in fans {
        fan.set_manual(false)?;
    }

    let pid_res = std::fs::remove_file(PID_FILE).map_err(Error::PidDelete);
    match (res, pid_res) {
        (Err(err), _) | (_, Err(err)) => Err(err),
        (Ok(()), Ok(())) => Ok(()),
    }
}
