use prql_compiler::sql::Dialect as D;
use rustler::{Atom, NifStruct};

mod atoms {
    rustler::atoms! {
      ok,
      error,

      // dialects
      ansi,
      big_query,
      click_house,
      generic,
      hive,
      mssql,
      mysql,
      postgres,
      sql_lite,
      snow_flake
    }
}

fn dialect_from_atom(a: Atom) -> prql_compiler::sql::Dialect {
    if a == atoms::ansi() {
        D::Ansi
    } else if a == atoms::big_query() {
        D::BigQuery
    } else if a == atoms::click_house() {
        D::ClickHouse
    } else if a == atoms::generic() {
        D::Generic
    } else if a == atoms::hive() {
        D::Hive
    } else if a == atoms::mssql() {
        D::MsSql
    } else if a == atoms::mysql() {
        D::MySql
    } else if a == atoms::postgres() {
        D::PostgreSql
    } else if a == atoms::sql_lite() {
        D::SQLite
    } else if a == atoms::snow_flake() {
        D::Snowflake
    } else {
        D::Generic
    }
}

impl From<CompileOptions> for prql_compiler::sql::Dialect {
    fn from(options: CompileOptions) -> Self {
        let dialect = options.dialect;
        match dialect {
            Some(d) => dialect_from_atom(d),
            None => D::Generic,
        }
    }
}

#[derive(Clone, NifStruct)]
#[module = "PRQL.Native.CompileOptions"]
pub struct CompileOptions {
    /// Pass generated SQL string trough a formatter that splits it
    /// into multiple lines and prettifies indentation and spacing.
    ///
    /// Defaults to true.
    pub format: bool,

    /// Target dialect you want to compile for.
    ///
    /// Because PRQL compiles to a subset of SQL, not all SQL features are
    /// required for PRQL. This means that generic dialect may work with most
    /// databases.
    ///
    /// If something does not work in dialect you need, please report it at
    /// GitHub issues.
    ///
    /// If None is used, `sql_dialect` flag from query definition is used.
    /// If it does not exist, [Dialect::Generic] is used.
    pub dialect: Option<Atom>,

    /// Emits the compiler signature as a comment after generated SQL
    ///
    /// Defaults to true.
    pub signature_comment: bool,
}

#[rustler::nif]
fn compile(prql_query: &str, options: Option<CompileOptions>) {}

rustler::init!("Elixir.PRQL", [compile]);
