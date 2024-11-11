# Soar Package Manager

<div align="center">

[crates-shield]: https://img.shields.io/crates/v/soar-cli
[crates-url]: https://crates.io/crates/soar-cli
[stars-shield]: https://img.shields.io/github/stars/pkgforge/soar.svg
[stars-url]: https://github.com/pkgforge/soar/stargazers
[issues-shield]: https://img.shields.io/github/issues/pkgforge/soar.svg
[issues-url]: https://github.com/pkgforge/soar/issues
[license-shield]: https://img.shields.io/github/license/pkgforge/soar.svg
[license-url]: https://github.com/pkgforge/soar/blob/main/LICENSE
[doc-shield]: https://img.shields.io/badge/docs-soar.qaidvoid.dev-blue
[doc-url]: https://soar.qaidvoid.dev

[![Crates.io][crates-shield]][crates-url]
[![Documentation][doc-shield]][doc-url]
[![Issues][issues-shield]][issues-url]
[![License: MIT][license-shield]][license-url]
[![Stars][stars-shield]][stars-url]

</div>

<p align="center">
    <img src="icons/hicolor/scalable/apps/soar.svg" alt="soar" width="256"/>
</p>

<p align="center">
    Soar is a fast Linux package manager that doesn't suck. Works with static binaries, AppImages, and other portable stuff.
</p>

<div align="center">

| <img src="https://raw.githubusercontent.com/pkgforge/soar/refs/heads/autoplay/install.webp" /> | <img src="https://raw.githubusercontent.com/pkgforge/soar/refs/heads/autoplay/remove.webp" /> | <img src="https://raw.githubusercontent.com/pkgforge/soar/refs/heads/autoplay/download.webp" /> | 
| - | - | - |
| **Install Packages** | **Remove Packages** | **Download File** |
| <img src="https://raw.githubusercontent.com/pkgforge/soar/refs/heads/autoplay/run.webp" /> | <img src="https://raw.githubusercontent.com/pkgforge/soar/refs/heads/autoplay/list.webp" /> | <img src="https://raw.githubusercontent.com/pkgforge/soar/refs/heads/autoplay/search.webp" /> |
| **Run Package** | **List Packages** | **Search Packages** |

</div>

## 🌟 Key Features
- [Universal Package Support](https://soar.qaidvoid.dev/#universal-package-support)
- [Desktop Integration](https://soar.qaidvoid.dev/#desktop-integration)

## 🔧 Installation
Installation guide can be found [here](https://soar.qaidvoid.dev/installation.html).

## 🎯 Usage

```sh
Usage: soar [OPTIONS] <COMMAND>

Commands:
  install    Install packages [aliases: i, add]
  search     Search package [aliases: s, find]
  query      Query package info [aliases: Q]
  remove     Remove packages [aliases: r, del]
  sync       Sync with remote metadata [aliases: S, fetch]
  update     Update packages [aliases: u, upgrade]
  info       Show info about installed packages [aliases: list-installed]
  list       List all available packages [aliases: ls]
  log        Inspect package build log
  inspect    Inspect package build script
  run        Run packages without installing to PATH [aliases: exec, execute]
  use        Use package from different family
  download   Download arbitrary files [aliases: dl]
  health     Health check
  defconfig  Generate default config
  env        View env
  help       Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  
  -q, --quiet       
  -j, --json        
  -h, --help        Print help
  -V, --version     Print version
```

## ⚙️ Configuration

Soar uses a JSON configuration file located at `~/.config/soar/config.json`.
For configuration guide, follow [here](https://soar.qaidvoid.dev/configuration.html).

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
