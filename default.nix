with import <nixpkgs> {};
mkShell {
  nativeBuildInputs = [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer
  ];
}
