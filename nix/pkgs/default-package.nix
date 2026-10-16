{ cake-unbaker, writeShellApplication }:
writeShellApplication {
  name = "nixpkgs-inventory";

  runtimeInputs = [
    cake-unbaker
  ];

  text = ''
    exec cake-unbaker get-flake-attributes "$@"
  '';
}
