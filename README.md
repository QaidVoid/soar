# 🚀 Soar Package Manager

Soar is a Linux package manager that doesn't suck. Works with static binaries, AppImages, and other portable stuff. It's fast too.

[![Crates.io](https://img.shields.io/crates/v/soar-cli)](https://crates.io/crates/soar-cli)
[![Documentation](https://img.shields.io/badge/docs-soar.qaidvoid.dev-blue)](https://soar.qaidvoid.dev)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 🌟 Key Features
- [Universal Package Support](https://soar.qaidvoid.dev/#universal-package-support)
- [Desktop Integration](https://soar.qaidvoid.dev/#desktop-integration)

## 🔧 Installation
Follow installation instructions at [installation.md](./docs/src/installation.md)

## 🎯 Usage

```sh
Usage: soar [OPTIONS] <COMMAND>

Commands:
  install   Install packages [aliases: i, add]
  search    Search package [aliases: s, find]
  query     Query package info [aliases: Q]
  remove    Remove packages [aliases: r, del]
  sync      Sync with remote metadata [aliases: S, fetch]
  update    Update packages [aliases: u, upgrade]
  info      Show info about installed packages [aliases: list-installed]
  list      List all available packages [aliases: ls]
  log       Inspect package build log
  inspect   Inspect package build script
  run       Run packages without installing to PATH [aliases: exec, execute]
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
For configuration guide, follow [configuration.md](./docs/src/configuration.md)

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
