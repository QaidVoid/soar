with import <nixpkgs> {};
mkShell {
  LD_LIBRARY_PATH = lib.makeLibraryPath [ openssl ];
  nativeBuildInputs = [
    openssl
    pkg-config
    rust-analyzer
  ];
}
