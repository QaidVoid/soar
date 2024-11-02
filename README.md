# 🚀 Soar Package Manager

A fast, modern package manager for Linux systems.

[![Under Development](https://img.shields.io/badge/status-under%20development-orange)](https://github.com/QaidVoid/soar)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


> **Note**: Soar is currently under rapid development.

## 🎯 Why Choose Soar?

- **Universal Package Support**: Unlike traditional package managers, Soar handles multiple package formats:
  - Binary packages
  - AppImages with automatic integration
  - FlatImages with desktop environment integration
  - More formats planned for future releases

- **Seamless Desktop Integration**: 
  - Automatic desktop entry creation
  - Icon integration across different resolutions
  - Smart symlink management
  - Portable home/config directory support for AppImages

## 🌟 Key Features

### Package Management
- **⚡ Lightning-Fast**: Parallel downloads and installations for maximum speed
- **🧰 Comprehensive Management**: Easy install, remove, update, and list operations
- **🌐 Multi-Repository Support**: Configure and use multiple package repositories
- **🔍 Smart Search**: Quickly find packages
- **🔄 Effortless Updates**: Keep your system up-to-date with a single command

### Advanced Features
- **🏃‍♂️ Run Without Install**: Try packages without permanent installation
- **📊 Detailed Information**: Get in-depth package info with image previews
- **🖼️ Image Support**: 
  - Sixel graphics protocol support
  - Kitty graphics protocol integration
  - HalfBlock image rendering

### Desktop Integration
- **🖥️ Automatic Desktop Entries**: Seamless integration with desktop environments
- **🎨 Icon Management**: Automatic icon scaling and integration
- **📁 Portable Configurations**: Support for portable home and config directories
- **🔗 Smart Symlink Handling**: Intelligent binary path management

## 🔧 Installation

### Using install script
```sh
curl -qfsSL "https://soar.qaidvoid.dev/install.sh" | sh
```

### From Source

1. Clone the repository:
```sh
git clone https://github.com/QaidVoid/soar.git
cd soar
```

2. Build and install using Cargo:
```sh
cargo build --release
cargo install --path .
```

### From Releases

1. Visit the [Releases](https://github.com/QaidVoid/soar/releases) page on GitHub.
2. Download the latest release for your platform.

## 🎯 Usage

```sh
Usage: soar [OPTIONS] <COMMAND>

Commands:
  install   Install packages [aliases: i]
  search    Search package [aliases: s]
  query     Query package info [aliases: Q]
  remove    Remove packages [aliases: r]
  sync      Sync with remote metadata [aliases: S]
  update    Update packages [aliases: u]
  info      Show info about installed packages
  list      List all available packages
  inspect   Inspect package build log
  run       Run packages without installing to PATH [aliases: exec]
  use       Use package from different family
  download  Download arbitrary files [aliases: dl]
  health    Health check
  help      Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose  Unimplemented
  -h, --help     Print help
  -V, --version  Print version
```

## ⚙️ Configuration

Soar uses a JSON configuration file located at `~/.config/soar/config.json`.

Default configuration
```json
{
  "soar_path": "$HOME/.soar",
  "repositories": [
    {
      "name": "ajam",
      "url": "https://bin.ajam.dev/x86_64",
      "metadata": "METADATA.AIO.json",
      "sources": {
        "bin": "https://bin.ajam.dev/x86_64",
        "pkg": "https://pkg.ajam.dev/x86_64",
        "base": "https://bin.ajam.dev/x86_64/Baseutils"
      }
    }
  ],
  "parallel": true,
  "parallel_limit": 2
}
```

### Configuration Fields

- `soar_path`: The path where Soar will store its data.

- `repositories`: Array of package repositories Soar will use to fetch packages.
  - `name`: A unique name for the repository.
  - `url`: The main URL of the repository.
  - `metadata`: The remote metadata filename.
  - `sources`: Specific URLs for different types of content within the repository.
    - `bin`: URL for downloading binary files.
    - `pkg`: URL for downloading package files.
    - `base`: URL for downloading base utility files.

- `parallel`: Boolean flag to enable or disable parallel downloads/installs.

- `parallel_limit`: The maximum number of concurrent downloads/installs when parallel mode is enabled.

You can customize these settings to fit you


## 🤝 Contributing

We welcome contributions! Please feel free to fork the repository and submit
pull requests. If you have suggestions or feature requests, open an issue to
discuss.

Please feel free to:
1. Fork the repository
2. Create your feature branch
3. Submit a pull request

## 📝 License

This project is licensed under [MIT] - see the [LICENSE](LICENSE) file for details.

