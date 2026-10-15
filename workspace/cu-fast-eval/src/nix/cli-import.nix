# Import following the standard nix-cli rules
# Given a path, imports it, and auto-calls with arguments if needed.
path: arguments:
let
  self = import path;
in
if builtins.isFunction self then self arguments else self
