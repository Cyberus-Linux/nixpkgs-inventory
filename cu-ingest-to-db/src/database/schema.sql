-- Evaluation of a Nixpkgs.
CREATE TABLE IF NOT EXISTS evaluation_informations (
  -- Identifier for the evaluation (multiple origins support in a single DB)
  id str PRIMARY KEY
  -- Free form field to save the expression input of the evaluation.
  -- More useful when dealing with a Nixpkgs source in the Nix store.
, expression json NOT NULL
  -- Arguments used at evaluation.
, arguments json NOT NULL
  -- Moment this evaluation was added to the database.
, created_at timestamp DEFAULT CURRENT_TIMESTAMP
);

-- Known derivations (`/nix/store/*.drv`)
-- Ties other tables together.
CREATE TABLE IF NOT EXISTS derivations (
  path store_path NOT NULL PRIMARY KEY
);

-- Nixpkgs information for attributes, for a given evaluation_informations(id).
CREATE TABLE IF NOT EXISTS evaluation_attributes (
  evaluation_id str NOT NULL
  -- Attribute path, as a list of strings (`["a", "b", "c"]`).
, attribute_path json NOT NULL
  -- Attribute path, as a string (`"a.b.c"`).
, attribute str NOT NULL
  -- Derivation the given attribute points to.
, derivation_path store_path NOT NULL
  -- `name` field of the derivation.
, name str NOT NULL
  -- `pname` when available.
, pname str
  -- `version` when available.
, version str
  -- `position` when available.
, position str
  -- The whole `meta` attrs when available.
, meta json
  -- The whole entry used to build this row.
, _raw json NOT NULL
, FOREIGN KEY (derivation_path) REFERENCES derivations(path)
, FOREIGN KEY (evaluation_id) REFERENCES evaluation_informations(id)
);
CREATE INDEX IF NOT EXISTS index_evaluation_attributes_derivation_path
  ON evaluation_attributes(derivation_path)
;

-- Derivations (input_derivation) that themselves are inputs to a given derivation (derivation_path).
CREATE TABLE IF NOT EXISTS derivation_input_derivations (
  derivation_path store_path NOT NULL
, input_derivation store_path NOT NULL
  -- output name used from the given input derivation.
  -- NOTE: unused for now (data currently missing from eval).
, output str NOT NULL
, PRIMARY KEY (derivation_path, input_derivation, output)
, FOREIGN KEY (derivation_path) REFERENCES derivations(path)
, FOREIGN KEY (input_derivation) REFERENCES derivations(path)
);
CREATE INDEX IF NOT EXISTS index_derivation_input_derivations_derivation_path
  ON derivation_input_derivations(derivation_path)
;

-- Store paths (derivation_path) of the outputs of a derivation (derivation_path).
CREATE TABLE IF NOT EXISTS derivation_output_paths (
  derivation_path store_path NOT NULL
, store_path str NOT NULL
, output str NOT NULL
, PRIMARY KEY (derivation_path, output)
, FOREIGN KEY (derivation_path) REFERENCES derivations(path)
);
CREATE INDEX IF NOT EXISTS index_derivation_output_paths_derivation_path
  ON derivation_output_paths(derivation_path)
;

-- Store paths (store_path) that are inputs to a given derivation (derivation_path).
CREATE TABLE IF NOT EXISTS derivation_input_paths (
  derivation_path store_path NOT NULL
, store_path str NOT NULL
, type str NOT NULL
, PRIMARY KEY (store_path, type)
, FOREIGN KEY (derivation_path) REFERENCES derivations(path)
);
CREATE INDEX IF NOT EXISTS index_derivation_input_paths_derivation_path
  ON derivation_input_paths(derivation_path)
;
