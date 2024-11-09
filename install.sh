#!/bin/sh
# Soar Package Manager Installation Script
# POSIX compliant installation script

set -eu

main() {
    DEFAULT_VERSION="latest"
    SOAR_VERSION="${SOAR_VERSION:-$DEFAULT_VERSION}"

    # Function to check for curl or wget
    check_download_tool() {
        if command -v curl >/dev/null 2>&1; then
            printf "curl -fsSL"
        elif command -v wget >/dev/null 2>&1; then
            printf "wget -qO-"
        else
            echo "Error: Neither curl nor wget found. Please install either curl or wget."
            exit 1
        fi
    }

    # Function to determine installation directory
    get_install_dir() {
        # Check environment variables first
        if [ -n "${SOAR_INSTALL_DIR-}" ]; then
            if [ -d "$SOAR_INSTALL_DIR" ] && [ -w "$SOAR_INSTALL_DIR" ]; then
                printf "%s" "$SOAR_INSTALL_DIR"
                return
            else
                echo "Error: SOAR_INSTALL_DIR ($SOAR_INSTALL_DIR) is not writable or doesn't exist"
                exit 1
            fi
        fi

        if [ -n "${INSTALL_DIR-}" ]; then
            if [ -d "$INSTALL_DIR" ] && [ -w "$INSTALL_DIR" ]; then
                printf "%s" "$INSTALL_DIR"
                return
            else
                echo "Error: INSTALL_DIR ($INSTALL_DIR) is not writable or doesn't exist"
                exit 1
            fi
        fi

        # Check ~/.local/bin
        local_bin="$HOME/.local/bin"
        if [ -d "$local_bin" ] && [ -w "$local_bin" ]; then
            printf "%s" "$local_bin"
            return
        fi

        # Fallback to /usr/local/bin if running as root
        if [ "$(id -u)" = "0" ]; then
            if [ -d "/usr/local/bin" ] && [ -w "/usr/local/bin" ]; then
                printf "/usr/local/bin"
                return
            fi
        fi

        # Fallback to current directory
        echo "Notice: ~/.local/bin not found or not writable. Installing in current directory." >&2
        echo "You should move the binary to a location in your \$PATH." >&2
        printf "%s" "$(pwd)"
    }

    # Function to download and install
    install_soar() {
        DOWNLOAD_TOOL=$(check_download_tool)
        INSTALL_PATH=$(get_install_dir)

        # Detect architecture
        ARCH=$(uname -m)
        case "$ARCH" in
            x86_64)
                ARCH="x86_64"
                ;;
            aarch64)
                ARCH="aarch64"
                ;;
            *)
                echo "Error: Unsupported architecture: $ARCH"
                exit 1
                ;;
        esac

        # Get latest release URL
        echo "Downloading Soar..."
        if [[ "$SOAR_VERSION" == *"nightly"* ]]; then
            RELEASE_URL="https://github.com/pkgforge/soar/releases/download/nightly/soar-nightly-$ARCH-linux"
        elif [[ "$SOAR_VERSION" == *"latest"* ]]; then
            RELEASE_URL="https://github.com/pkgforge/soar/releases/latest/download/soar-$ARCH-linux"
        else
            RELEASE_URL="https://github.com/pkgforge/soar/releases/download/v$SOAR_VERSION/soar-$ARCH-linux"
        fi
        echo $RELEASE_URL

        # Download and install
        $DOWNLOAD_TOOL "$RELEASE_URL" > "$INSTALL_PATH/soar"

        if [ ! -f "$INSTALL_PATH/soar" ]; then
            echo "Error: Download failed"
            exit 1
        fi

        # Make executable
        chmod +x "$INSTALL_PATH/soar"

        # Run health check
        echo "Running health check..."
        if ! "$INSTALL_PATH/soar" health; then
            echo "Warning: Health check failed. Please check your installation."
        fi

        echo "Soar has been installed to: $INSTALL_PATH/soar"
        echo "Make sure $INSTALL_PATH is in your PATH."
    }

    # Execute installation
    install_soar
}

# Call main function
main
