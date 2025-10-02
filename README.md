# T2FanRD

Simple Fan Daemon for T2 Macs, rewritten from the [original Python version](https://github.com/NoaHimesaka1873/t2fand).

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

### NixOS
Find out how many fans you have (using lm_sensors for example, as in: `$ nix-shell -p lm_sensors --run "sensors | grep fan"`).

Configure each fan with `services.t2fanrd.config.<name-of-fan>` using the options from the Standard configuration above. See the example declared with the option in the flake.

Start the systemd service with `services.t2fanrd.enable = true;`.
