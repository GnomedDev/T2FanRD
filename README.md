# T2FanRD
Simple Fan Daemon for T2 Macs.

Rewritten from the [original Python version](https://github.com/NoaHimesaka1873/t2fand).
Rust version created by [GnomedDev](https://github.com/GnomedDev/T2FanRD)
Initial flake created by [soopyc](https://github.com/soopyc/T2FanRD).
Updated with nix configuration support by [ethanmoss1](https://github.com/ethanmoss1).

## Compilation
`cargo build --release`

## Installation
### Standard
1. Copy the `target/release/t2fanrd` executable to wherever your distro wants executables to be run by root.
2. Setup the executable to be run automatically at startup, like via a [systemd service](https://github.com/t2linux/fedora/blob/2947fdc909a35f04eb936a4f9c0f33fe4e52d9c2/t2fanrd/t2fanrd.service).

### Nixos
Add the flake to you inputs and outputs, ensuring to import the nixosModules.
```nix
{
  inputs {
    t2fanrd.url = "github:soopyc/T2FanRD/master";
  };

  outputs = {
      t2fanrd,
  }: {
    nixosConfiguration.yourhost = nixpkgs.lib.nixosSystem {
      modules = [
        t2fanrd.nixosModules.t2fanrd
      ]
    };
  };
}
```
Note: a lot of the rest of the flake is missing, make sure you understand the basic structure of a flake first.

## Configuration
### Config File
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

### Nixos
With the understanding of the configuration above, you can apply this to your nixos configuration with the supplied module options.

In your configuration.nix (or one of your imported modules), add the nixos options for enabling the T2fanrd service. Configure each of the fans on your machine with submodules, each fan follows the same configuration rules as described above;
```nix
{}:
{
  services.t2fanrd = {
    enable = true;
    fans = [
      # Configure each fan here;
      {
        name = "Fan1";
        lowTemp = 40;
        highTemp = 70;
        speedCurve = "linear";
        alwaysFullSpeed = false;
      }
      {
        name = "Fan2";
        lowTemp = 40;
        highTemp = 70;
        speedCurve = "linear";
        alwaysFullSpeed = false;
      }
    ];
  };
}

```
