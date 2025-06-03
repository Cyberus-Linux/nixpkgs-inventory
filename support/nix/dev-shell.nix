{ mkShell
, cargo
, rustc
, clippy
, rustfmt
}:
mkShell {
  nativeBuildInputs = [
    cargo
    rustc
    clippy
    rustfmt
  ];
}
