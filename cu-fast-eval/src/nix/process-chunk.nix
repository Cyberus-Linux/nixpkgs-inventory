/* Expression for processing chunks. */
{ attrs
, chunkPath
}:

let
  # Check if given value is a derivation.
  isDerivation = value: (value.type or null) == "derivation";

  hasOutPath =
    value:
    builtins.isAttrs value && value ? outPath
  ;

  # Parse JSON from file.
  readJSON = path: builtins.fromJSON ( builtins.readFile path );

  # Given a list, concatenates and flattens the lists or attrsets, recursively
  getPathsAndStringsDeep =
    list:
    builtins.foldl'
    (acc: curr:
      acc ++ (
        if builtins.isList curr then
          # Pass through lists
          getPathsAndStringsDeep curr
        else if builtins.isString curr || builtins.isPath curr || isDerivation curr || hasOutPath curr then
          # Strings and paths are wrapped in a list.
          [ "${curr}" ]
        else if builtins.isAttrs curr then
          # Pick values out of attrsets.
          getPathsAndStringsDeep (builtins.attrValues curr)
        else
         [ /* ignore other values */ ]
      )
    )
    []
    list
  ;

  # Get an attribute from nested attribute sets, by path.
  # path: list of attribute names to get a value from nested attribute sets
  # attrs: attribute set on which to get a value from path
  getAttrFromPath =
    path: attrs:
    (
      builtins.foldl'
      (
        # For every component found in `path`
        result: component:
        {
          # Keep the current path around for the next foldl' application.
          # (This will be used for the error message.)
          path  = result.path ++ [ component ];
          # Try to get the corresponding value
          attrs =
            result.attrs."${component}" or (
              # Fail with a useful message.
              builtins.throw "attribute ${builtins.toJSON component} not found on ${builtins.toJSON result.path}."
            )
          ;
        }
      )
      { inherit attrs; path = []; }
      path
    ).attrs
  ;

  # All attributes to check for this evaluation chunk.
  attrPaths = readJSON chunkPath;

  # Strip failed evaluations from the raw evaluation.
  # (This is the returned value.)
  results =
    map
    (el: el.value)
    (builtins.filter (el: el.success) eval)
  ;

  # Raw evaluation, full of `tryEval` results.
  # This needs to be filtered for successful evals.
  eval =
    map
    (attrPath:
      builtins.tryEval (
        let
          nonDerivationMsg = "warning: Unexpectedly found a ${builtins.typeOf deriv} instead of a derivation at ${builtins.toJSON attrPath}.";
          deriv = getAttrFromPath attrPath attrs;
          result =
            if !(isDerivation deriv)
            then
              # Trace so the message is shown, and also throw, so tryEval fails.
              (builtins.trace nonDerivationMsg (builtins.throw nonDerivationMsg))
            else
              {
                inherit attrPath;
                inherit (deriv)
                  drvPath
                  name
                ;
                pname = deriv.pname or null;
                version = deriv.version or null;

                # Direct inputs
                #
                # NOTE: other inputs are handled using `nix-store --query --references`.
                #
                # Both `src` and `srcs` in Nixpkgs can be any of:
                #  - Not set
                #  - a string
                #  - a list
                #    - of strings
                #    - of lists [...]
                # This is why we're combining `src` and `srcs` as `srcs`.
                srcs =
                  []
                  ++ (getPathsAndStringsDeep [ (deriv.src or null) ])
                  ++ (getPathsAndStringsDeep [ (deriv.srcs or null) ])
                ;
                patches = getPathsAndStringsDeep [ (deriv.patches or null) ];

                # Outputs
                outputs =
                    builtins.listToAttrs (
                      map
                      (name: {
                        inherit name;
                        value = deriv."${name}";
                      })
                      # If `outputs` is not set, we're synthesizing it.
                      (deriv.outputs or [ deriv.outputName ])
                    )
                ;
                # Get the meta from the derivation
                meta = deriv.meta or null;
              }
          ;
        in
        # This `seq` ensures the derivation itself has been evaluated during the `tryEval`.
        # Otherwise this returns values that are still lazily to-be-evaluated.
        # The yet-to-be-evaluated value may then fail *outside* of the `tryEval` context, failing the whole eval.
        builtins.seq "${deriv}" result
        # NOTE: using `builtins.deepSeq` on the `result` here will cause a stack overflow.
        #       evaluating the derivation itself is sufficient to trigger the meta checks and such.
        #       Unverified, but this is likely coming from packages “invalid” for the platform.
        #       They may depend on other packages causing a cycle in dependencies, even though they are not valid.
        #       The check for validity (e.g. platforms) happens "outside" of the evaluation of the actual derivation.
        #       The `outputs` attrset cause the actual derivation to be evaluated.
      )
    )
    attrPaths
  ;
in
  results
