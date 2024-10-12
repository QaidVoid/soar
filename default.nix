with import <nixpkgs> {};
mkShell {
  nativeBuildInputs = [
    rust-analyzer
  ];
}
