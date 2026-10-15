use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

// Re-exporting, leaky abstraction.
pub use rusqlite::named_params;

const DATABASE_SCHEMA: &str = include_str!("database/schema.sql");

/// Represents the connection to the database.
/// Used for the `impl`.
#[derive(Clone)]
pub struct AttributesDatabase {
    connection: Arc<Mutex<Connection>>,
}

/// Encapsulate tables we know about
pub enum Table {
    EvaluationInformations,
    Derivations,
    EvaluationAttributes,
    DerivationInputDerivations,
    DerivationOutputPaths,
    DerivationInputPaths,
}

/// Encapsulates information about said tables
impl Table {
    /// Returns the table name for the given table.
    pub fn get_name(&self) -> &str {
        match self {
            Table::EvaluationInformations => "evaluation_informations",
            Table::Derivations => "derivations",
            Table::EvaluationAttributes => "evaluation_attributes",
            Table::DerivationInputDerivations => "derivation_input_derivations",
            Table::DerivationOutputPaths => "derivation_output_paths",
            Table::DerivationInputPaths => "derivation_input_paths",
        }
    }
    /// Returns the columns for the given table.
    /// May not include all columns (i.e. created_at)
    pub fn get_columns(&self) -> Vec<&str> {
        match self {
            Table::EvaluationInformations => vec!["id", "expression", "arguments"],
            Table::Derivations => vec!["path"],
            Table::EvaluationAttributes => vec![
                "evaluation_id",
                "attribute_path",
                "attribute",
                "derivation_path",
                "name",
                "pname",
                "version",
                "position",
                "meta",
                "_raw",
            ],
            Table::DerivationInputDerivations => {
                vec!["derivation_path", "input_derivation", "output"]
            }
            Table::DerivationOutputPaths => vec!["derivation_path", "store_path", "output"],
            Table::DerivationInputPaths => vec!["derivation_path", "store_path", "type"],
        }
    }
}

/// `INSERT` query types
pub enum InsertDirective {
    Insert,
    InsertOrIgnore,
    InsertOrReplace,
}

impl InsertDirective {
    fn to_sql(&self) -> &str {
        match self {
            InsertDirective::Insert => "INSERT",
            InsertDirective::InsertOrIgnore => "INSERT OR IGNORE",
            InsertDirective::InsertOrReplace => "INSERT OR REPLACE",
        }
    }
}

/// `INSERT` query configuration.
pub struct InsertQuery {
    pub directive: InsertDirective,
    pub table: Table,
}

/// Quick helpers managing the database bits more ergonomically.
impl AttributesDatabase {
    pub fn open(path: &String) -> Result<AttributesDatabase> {
        let connection = Connection::open(path)?;
        connection.execute_batch(DATABASE_SCHEMA)?;
        connection.pragma_update(None, "auto_vacuum", "none")?;
        connection.pragma_update(None, "count_changes", false)?;
        connection.pragma_update(None, "journal_mode", "memory")?;
        connection.pragma_update(None, "synchronous", "off")?;
        connection.pragma_update(None, "temp_store", "memory")?;

        // Wrap into the mutex
        let connection = Mutex::new(connection);

        Ok(AttributesDatabase {
            connection: connection.into(),
        })
    }

    pub fn query_map<T, P: rusqlite::Params, F: FnMut(&rusqlite::Row<'_>) -> Result<T>>(
        &self,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<Vec<T>> {
        Ok(self
            .connection
            .lock()
            .unwrap()
            .prepare(sql)?
            .query_map(params, f)?
            .map(|r| r.unwrap())
            .collect())
    }

    pub fn query_one<T, P: rusqlite::Params, F: FnOnce(&rusqlite::Row<'_>) -> Result<T>>(
        &self,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<T> {
        self.connection.lock().unwrap().query_one(sql, params, f)
    }

    pub fn execute<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<usize, rusqlite::Error> {
        self.connection.lock().unwrap().execute(sql, params)
    }

    /// Handles the boring part of simple INSERT queries.
    /// It is valid not to provide all the named parameters.
    ///
    /// NOTE: Due to the simple implementation, parameter names are not checked at compile-time.
    pub fn insert<P: rusqlite::Params>(
        &self,
        from: InsertQuery,
        params: P,
    ) -> Result<usize, rusqlite::Error> {
        let query = format!(
            "{0} INTO {1} ({2}) VALUES ({3});",
            from.directive.to_sql(),
            from.table.get_name(),
            from.table.get_columns().join(", "),
            from.table
                .get_columns()
                .iter()
                .map(|s| format!(":{s}"))
                .collect::<Vec<String>>()
                .join(", "),
        );

        self.execute(&query, params)
    }

    pub fn has_evaluation(&self, id: &String) -> bool {
        let result = self
            .query_one(
                &format!(
                    "SELECT COUNT(*) FROM {} WHERE id = :id",
                    Table::EvaluationInformations.get_name()
                ),
                named_params! {
                    ":id": &id,
                },
                |row| row.get::<usize, usize>(0),
            )
            .unwrap();

        result > 0
    }
}

/// Shortcut for AttributesDatabase::open()
pub fn open(path: &String) -> Result<AttributesDatabase> {
    AttributesDatabase::open(path)
}
