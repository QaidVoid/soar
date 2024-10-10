# 🚀 Soar Package Manager

A fast, modern package manager for Linux systems.

[![Under Development](https://img.shields.io/badge/status-under%20development-orange)](https://github.com/QaidVoid/soar)

> **Note**: Soar is currently under rapid development.

## 🌟 Features

- **Fast Installation**: Parallel package downloads and installations
- **Package Management**: Install, remove, update, and list packages effortlessly
- **Repository Support**: Multiple repository configurations

## 🔧 Installation

```bash
cargo install --path .
```

## 🎯 Usage

```bash
Usage: soar [OPTIONS] <COMMAND>

Commands:
  install  Install packages; supports '--force' flag [aliases: i]
  search   Search package [aliases: s]
  query    Query package info [aliases: Q]
  remove   Remove packages [aliases: r]
  sync     Sync with remote registry [aliases: S]
  update   Update packages [aliases: u]
  info     Show info about installed packages
  list     List all available packages
  inspect  Inspect package build log
  run      Run packages without installing to PATH [aliases: exec]
  use      Use different variant of a package
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose  Unimplemented
  -h, --help     Print help
  -V, --version  Print version
```

## 🤝 Contributing

We welcome contributions! Please feel free to fork the repository and submit
pull requests. If you have suggestions or feature requests, open an issue to
discuss.

## 📝 License

This project is licensed under [MIT] - see the [LICENSE](LICENSE) file for details.
