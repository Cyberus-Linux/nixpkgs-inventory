{ pkg-config
, sqlite
, nix
}:

# Generic subproject attributes
{ buildInputs ? []
, nativeBuildInputs ? []
, ...
}:

{
  COMMAND_NIX_STORE = "${nix}/bin/nix-store";
  buildInputs = buildInputs ++ [
    sqlite
  ];
  nativeBuildInputs = nativeBuildInputs ++ [
    pkg-config
  ];
}
