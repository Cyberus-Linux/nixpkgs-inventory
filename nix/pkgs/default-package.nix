{ cake-unbaker, writeShellApplication }:
writeShellApplication {
  name = "get-flake-attributes";

  runtimeInputs = [
    cake-unbaker
  ];

  text = ''
    exec cake-unbaker get-flake-attributes "$@"
  '';
}
