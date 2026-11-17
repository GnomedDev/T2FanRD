# T2FanRD

Simple Fan Daemon for T2 Macs, rewritten from the [original Python version](https://github.com/NoaHimesaka1873/t2fand).

## Archived

This repository has been archived due to it's purpose being fulfilled, with recent kernels supporting the standard hwmon interface.

Standard userspace fan daemons should work, although `1` needs to be written to `/sys/module/macsmc_hwmon/parameters/fan_control` first.

For those who still want to use the `t2fand` config format or behaviour, see the following fork: https://github.com/sharpenedblade/T2FanRD

## Compilation
`cargo build --release`

## Installation
### Standard
1. Copy the `target/release/t2fanrd` executable to wherever your distro wants executables to be run by root.
2. Setup the executable to be run automatically at startup, like via a [systemd service](https://github.com/t2linux/fedora/blob/2947fdc909a35f04eb936a4f9c0f33fe4e52d9c2/t2fanrd/t2fanrd.service).

### NixOS
Add this repo to your flake inputs, and the module `<name-of-input>.nixosModule.t2fanrd` to your imports.

## Configuration
### Standard
Initial configuration will be done automatically.

For manual config, the config file can be found at `/etc/t2fand.conf`.

There's four options for each fan.
|        Key        |                            Value                            |
|:-----------------:|:-----------------------------------------------------------:|
|      low_temp     |        Temperature that will trigger higher fan speed       |
|     high_temp     |         Temperature that will trigger max fan speed         |
|    speed_curve    |   Three options present. Will be explained in table below.  |
| always_full_speed | if set "true", the fan will be at max speed no matter what. |

For `speed_curve`, there's three options.
|     Key     |                   Value                   |
|:-----------:|:-----------------------------------------:|
|    linear   |     Fan speed will be scaled linearly.    |
| exponential |  Fan speed will be scaled exponentially.  |
| logarithmic | Fan speed will be scaled logarithmically. |

Here's an image to better explain this. (Red: linear, blue: exponential, green: logarithmic)
![Image of fan curve graphs](https://user-images.githubusercontent.com/39993457/233580720-cfdaba12-a2d8-430c-87a2-15209dcfec6d.png)

### Extra temperature sensors
By default the fans follow the hotter of the CPU and the GPU. Other parts can heat up while those
stay cool, for example an NVMe drive under a long write while the CPU idles: on a Mac Pro 2019 a
drive reached 73 to 76 C with the fans at minimum, and 61 C once this sensor raised them.

Optional `[Sensor1]`, `[Sensor2]`, ... sections add such sensors. Each has its own curve, and
every fan runs at the highest speed asked for by its own `[FanN]` curve or by any sensor.
|     Key     |                                         Value                                         |
|:-----------:|:-------------------------------------------------------------------------------------:|
|     path    | Glob of hwmon `temp*_input` files. All matches are read and the hottest one counts.   |
|   low_temp  |                     Temperature from which this sensor raises the fans                  |
|  high_temp  |                    Temperature at which this sensor asks for max fan speed              |
| speed_curve |              Optional, `linear` (default), `exponential` or `logarithmic`               |

Example for all NVMe drives:
```ini
[Sensor1]
path=/sys/class/nvme/nvme*/hwmon*/temp1_input
low_temp=50
high_temp=70
```

A sensor that matches nothing at startup is skipped with a message; one that becomes unreadable
later is dropped and the fans keep following the others.

### NixOS
Find out how many fans you have (using lm_sensors for example, as in: `$ nix-shell -p lm_sensors --run "sensors | grep fan"`).

Configure each fan with `services.t2fanrd.config.<name-of-fan>` using the options from the Standard configuration above. See the example declared with the option in the flake.

Start the systemd service with `services.t2fanrd.enable = true;`.
