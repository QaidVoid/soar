# 🚀 Soar Package Manager

A fast, modern package manager for Linux systems.

[![Under Development](https://img.shields.io/badge/status-under%20development-orange)](https://github.com/QaidVoid/soar)

> **Note**: Soar is currently under rapid development. The codebase is being
actively refactored and improved. New features will be added after a
comprehensive restructuring of the core components.

## 🌟 Features

- **Fast Installation**: Parallel package downloads and installations
- **Package Management**: Install, remove, update, and list packages effortlessly
- **Repository Support**: Multiple repository configurations

## 🚀 Current Capabilities

- `install` - Install packages from configured repositories
- `search` - Search for available packages
- `list` - List installed packages
- `remove` - Remove installed packages
- `update` - Update installed packages (currently using install methods internally)

## 🔧 Installation

```bash
cargo install --path .
```

## 🎯 Usage

```bash
soar fetch                     # Fetch and update metadata
soar install <package-names>   # Install package(s)
soar search <package-name>     # Search for a package
soar list                      # List installed packages
soar remove <package-names>    # Remove package(s)
soar update [package-names]    # Update package(s)
```

## 🚧 Development Status

Soar is currently in active development.

- The codebase is undergoing significant refactoring
- Some features (like update) don't work as expected
- New features are planned post-refactoring

## 🤝 Contributing

Currently, contributions are not accepted as the project is undergoing a heavy
refactor. Interest is appreciated, and contributions will be welcomed once the
refactoring is complete. Suggestions and feedback will be invaluable at that
time!

## 📝 License

This project is licensed under [MIT] - see the [LICENSE](LICENSE) file for details.
