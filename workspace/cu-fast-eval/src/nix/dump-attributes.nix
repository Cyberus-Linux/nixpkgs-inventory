/* Expression for dumping attributes. */
let
  traverseAttrs =
    { attrs
    , filter
    , recurse
    , curr ? []
    }:
    builtins.concatLists (
      builtins.map
      (
        name:
        let value = (builtins.tryEval (attrs.${name})).value; in
        (
        if filter == null || filter value
        then
          [ ( curr ++ [name]) ]
        else
          [ /* drop attribute name */ ]
        )
        ++
        (
        if (builtins.isFunction recurse) && (builtins.isAttrs value) && (recurse value)
        then
          traverseAttrs {
            attrs = value;
            inherit filter recurse;
            curr = curr ++ [name];
          }
        else
          []
        )
      )
      (builtins.attrNames attrs)
    )
  ;
in
# We are opting into these arguments, instead of exposing the traverseAttrs API.
{ attrs
, filter ? null
, recurse ? null
}:
traverseAttrs { inherit attrs filter recurse; }
