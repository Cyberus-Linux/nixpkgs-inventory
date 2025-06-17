{ pkg-config
, sqlite
}:

# Generic subproject attributes
{ buildInputs ? []
, nativeBuildInputs ? []
, ...
}:

{
  buildInputs = buildInputs ++ [
    sqlite
  ];
  nativeBuildInputs = nativeBuildInputs ++ [
    pkg-config
  ];
}
