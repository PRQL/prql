//! Simple tests for "this PRQL creates this SQL" go here.
use insta::{assert_display_snapshot, assert_snapshot};
use prql_compiler::{sql, ErrorMessages, Options, SourceTree, Target};
use rstest::rstest;

pub fn compile(prql: &str) -> Result<String, ErrorMessages> {
    anstream::ColorChoice::Never.write_global();
    prql_compiler::compile(prql, &Options::default().no_signature())
}

fn compile_with_sql_dialect(prql: &str, dialect: sql::Dialect) -> Result<String, ErrorMessages> {
    anstream::ColorChoice::Never.write_global();
    prql_compiler::compile(
        prql,
        &Options::default()
            .no_signature()
            .with_target(Target::Sql(Some(dialect))),
    )
}

#[test]
fn test_stdlib() {
    assert_snapshot!(compile(r###"
    from employees
    aggregate (
        {salary_usd = min salary}
    )
    "###).unwrap(),
        @r###"
    SELECT
      MIN(salary) AS salary_usd
    FROM
      employees
    "###
    );

    assert_snapshot!(compile(r###"
    from employees
    aggregate (
        {salary_usd = (math.round 2 salary)}
    )
    "###).unwrap(),
        @r###"
    SELECT
      ROUND(salary, 2) AS salary_usd
    FROM
      employees
    "###
    );
}

#[test]
fn test_stdlib_math_module() {
    assert_snapshot!(compile(r#"
    from employees
    select {
      salary_abs = math.abs salary,
      salary_floor = math.floor salary,
      salary_ceil = math.ceil salary,
      salary_pi = math.pi,
      salary_exp = math.exp salary,
      salary_ln = math.ln salary,
      salary_log10 = math.log10 salary,
      salary_log = math.log 2 salary,
      salary_sqrt = math.sqrt salary,
      salary_degrees = math.degrees salary,
      salary_radians = math.radians salary,
      salary_cos = math.cos salary,
      salary_acos = math.acos salary,
      salary_sin = math.sin salary,
      salary_asin = math.asin salary,
      salary_tan = math.tan salary,
      salary_atan = math.atan salary,
      salary_pow = salary | math.pow 2,
    }
    "#).unwrap(), @r#"
    SELECT
      ABS(salary) AS salary_abs,
      FLOOR(salary) AS salary_floor,
      CEIL(salary) AS salary_ceil,
      PI() AS salary_pi,
      EXP(salary) AS salary_exp,
      LN(salary) AS salary_ln,
      LOG10(salary) AS salary_log10,
      LOG10(salary) / LOG10(2) AS salary_log,
      SQRT(salary) AS salary_sqrt,
      DEGREES(salary) AS salary_degrees,
      RADIANS(salary) AS salary_radians,
      COS(salary) AS salary_cos,
      ACOS(salary) AS salary_acos,
      SIN(salary) AS salary_sin,
      ASIN(salary) AS salary_asin,
      TAN(salary) AS salary_tan,
      ATAN(salary) AS salary_atan,
      POW(salary, 2) AS salary_pow
    FROM
      employees
    "#
    );
}

#[test]
fn test_stdlib_math_module_mssql() {
    assert_snapshot!(compile(r#"
  prql target:sql.mssql

  from employees
  select {
    salary_abs = math.abs salary,
    salary_floor = math.floor salary,
    salary_ceil = math.ceil salary,
    salary_pi = math.pi,
    salary_exp = math.exp salary,
    salary_ln = math.ln salary,
    salary_log10 = math.log10 salary,
    salary_log = math.log 2 salary,
    salary_sqrt = math.sqrt salary,
    salary_degrees = math.degrees salary,
    salary_radians = math.radians salary,
    salary_cos = math.cos salary,
    salary_acos = math.acos salary,
    salary_sin = math.sin salary,
    salary_asin = math.asin salary,
    salary_tan = math.tan salary,
    salary_atan = math.atan salary,
    salary_pow = salary | math.pow 2,
  }
  "#).unwrap(), @r#"
  SELECT
    ABS(salary) AS salary_abs,
    FLOOR(salary) AS salary_floor,
    CEILING(salary) AS salary_ceil,
    PI() AS salary_pi,
    EXP(salary) AS salary_exp,
    LOG(salary) AS salary_ln,
    LOG10(salary) AS salary_log10,
    LOG10(salary) / LOG10(2) AS salary_log,
    SQRT(salary) AS salary_sqrt,
    DEGREES(salary) AS salary_degrees,
    RADIANS(salary) AS salary_radians,
    COS(salary) AS salary_cos,
    ACOS(salary) AS salary_acos,
    SIN(salary) AS salary_sin,
    ASIN(salary) AS salary_asin,
    TAN(salary) AS salary_tan,
    ATAN(salary) AS salary_atan,
    POWER(salary, 2) AS salary_pow
  FROM
    employees
  "#
    );
}

#[test]
fn test_stdlib_text_module() {
    assert_snapshot!(compile(r#"
    from employees
    select {
      name_lower = name | text.lower,
      name_upper = name | text.upper,
      name_ltrim = name | text.ltrim,
      name_rtrim = name | text.rtrim,
      name_trim = name | text.trim,
      name_length = name | text.length,
      name_extract = name | text.extract 3 5,
      name_replace = name | text.replace "pika" "chu",
      name_starts_with = name | text.starts_with "pika",
      name_contains = name | text.contains "pika",
      name_ends_with = name | text.ends_with "pika",
    }
    "#).unwrap(), @r###"
    SELECT
      LOWER(name) AS name_lower,
      UPPER(name) AS name_upper,
      LTRIM(name) AS name_ltrim,
      RTRIM(name) AS name_rtrim,
      TRIM(name) AS name_trim,
      CHAR_LENGTH(name) AS name_length,
      SUBSTRING(name, 3, 5) AS name_extract,
      REPLACE(name, 'pika', 'chu') AS name_replace,
      name LIKE CONCAT('pika', '%') AS name_starts_with,
      name LIKE CONCAT('%', 'pika', '%') AS name_contains,
      name LIKE CONCAT('%', 'pika') AS name_ends_with
    FROM
      employees
    "###
    );
}

#[rstest]
#[case::generic(sql::Dialect::Generic, "LIKE CONCAT('%', 'pika', '%')")]
#[case::sqlite(sql::Dialect::SQLite, "LIKE '%' || 'pika' || '%'")] // `CONCAT` is not supported in SQLite
fn like_concat(#[case] dialect: sql::Dialect, #[case] expected_like: &'static str) {
    let query = r#"
  from employees
  select {
    name_ends_with = name | text.contains "pika",
  }
  "#;
    let expected = format!(
        r#"
SELECT
  name {expected_like} AS name_ends_with
FROM
  employees
"#
    );
    assert_eq!(
        compile_with_sql_dialect(query, dialect).unwrap(),
        expected.trim_start()
    )
}

#[rstest]
#[case::clickhouse(
    sql::Dialect::ClickHouse,
    "formatDateTimeInJodaSyntax(invoice_date, 'dd/MM/yyyy')"
)]
#[case::duckdb(sql::Dialect::DuckDb, "strftime(invoice_date, '%d/%m/%Y')")]
#[case::postgres(sql::Dialect::Postgres, "TO_CHAR(invoice_date, 'DD/MM/YYYY')")]
#[case::mssql(sql::Dialect::MsSql, "FORMAT(invoice_date, 'dd/MM/yyyy')")]
#[case::mysql(sql::Dialect::MySql, "DATE_FORMAT(invoice_date, '%d/%m/%Y')")]
fn date_to_text_operator(
    #[case] dialect: sql::Dialect,
    #[case] expected_date_to_text: &'static str,
) {
    let query = r#"
    from invoices
    select {
      invoice_date = invoice_date | date.to_text "%d/%m/%Y"
    }"#;
    let expected = format!(
        r#"
SELECT
  {expected_date_to_text} AS invoice_date
FROM
  invoices
"#
    );
    assert_eq!(
        compile_with_sql_dialect(query, dialect).unwrap(),
        expected.trim_start()
    )
}

#[test]
fn json_of_test() {
    let json = prql_compiler::prql_to_pl("from employees | take 10")
        .and_then(prql_compiler::json::from_pl)
        .unwrap();
    // Since the AST is so in flux right now just test that the brackets are present
    assert_eq!(json.chars().next().unwrap(), '[');
    assert_eq!(json.chars().nth(json.len() - 1).unwrap(), ']');
}

#[test]
fn test_precedence_division() {
    assert_display_snapshot!((compile(r###"
    from artists
    derive {
      p1 = a - (b + c), # needs parentheses
      p2 = x / (y * z), # needs parentheses
      np1 = x / y / z, # doesn't need parentheses
      p3 = x / (y / z), # needs parentheses
      np4 = (x / y) / z, # doesn't need parentheses
    }
    "###).unwrap()), @r###"
    SELECT
      *,
      a - (b + c) AS p1,
      x / (y * z) AS p2,
      x / y / z AS np1,
      x / (y / z) AS p3,
      x / y / z AS np4
    FROM
      artists
    "###);
}

#[test]
fn test_precedence() {
    assert_display_snapshot!((compile(r###"
    from artists
    derive {
      p1 = a - (b + c), # needs parentheses
      p2 = a / (b * c), # needs parentheses
      np1 = a + (b - c), # no parentheses
      np2 = (a + b) - c, # no parentheses
    }
    "###).unwrap()), @r###"
    SELECT
      *,
      a - (b + c) AS p1,
      a / (b * c) AS p2,
      a + b - c AS np1,
      a + b - c AS np2
    FROM
      artists
    "###);

    assert_display_snapshot!((compile(r###"
    from x
    derive {
      temp_c = (temp_f - 32) / 1.8,
      temp_f = temp_c * 9/5,
      temp_z = temp_x + 9 - 5,
    }
    "###).unwrap()), @r###"
    SELECT
      *,
      (temp_f - 32) / 1.8 AS temp_c,
      (temp_f - 32) / 1.8 * 9 / 5 AS temp_f,
      temp_x + 9 - 5 AS temp_z
    FROM
      x
    "###);

    assert_display_snapshot!((compile(r###"
    from numbers
    derive {
      sum_1 = a + b, 
      sum_2 = add a b,
      g = -a
    }
    select {
      result = c * sum_1 + sum_2,
      a * g
    }
    "###).unwrap()), @r###"
    SELECT
      c * (a + b) + a + b AS result,
      a * - a
    FROM
      numbers
    "###);

    assert_display_snapshot!((compile(r###"
    from comparisons
    select {
      gtz = a > 0,
      ltz = !(a > 0),
      zero = !gtz && !ltz,
      is_not_equal = !(a==b),
      is_not_gt = !(a>b),
      negated_is_null_1 = !a == null,
      negated_is_null_2 = (!a) == null,
      is_not_null = !(a == null),
      (a + b) == null,
    }
    "###).unwrap()), @r###"
    SELECT
      a > 0 AS gtz,
      NOT a > 0 AS ltz,
      NOT a > 0
      AND NOT NOT a > 0 AS zero,
      NOT a = b AS is_not_equal,
      NOT a > b AS is_not_gt,
      (NOT a) IS NULL AS negated_is_null_1,
      (NOT a) IS NULL AS negated_is_null_2,
      NOT a IS NULL AS is_not_null,
      a + b IS NULL
    FROM
      comparisons
    "###);

    assert_display_snapshot!(compile(
    r###"
    from numbers
    derive x = (y - z)
    select {
      c - (a + b),
      c + (a - b),
      c + a - b,
      c + a + b,
      (c + a) - b,
      ((c - d) - (a - b)),
      ((c + d) + (a - b)),
      a / (b * c),
      +x,
      -x,
    }
    "###
    ).unwrap(), @r###"
    SELECT
      c - (a + b),
      c + a - b,
      c + a - b,
      c + a + b,
      c + a - b,
      c - d - (a - b),
      c + d + a - b,
      a / (b * c),
      y - z AS x,
      -(y - z)
    FROM
      numbers
    "###
    );
}

#[test]
#[ignore]
// FIXME: right associativity of `pow` is not implemented yet
fn test_pow_is_right_associative() {
    assert_display_snapshot!(compile(r#"
    from numbers
    select {
      c ** a ** b
    }
    "#).unwrap(), @r#"
    SELECT
      POW(c, POW(a, b))
    FROM
      numbers
    "#
    );
}

#[test]
fn test_append() {
    assert_display_snapshot!(compile(r###"
    from employees
    append managers
    "###).unwrap(), @r###"
    SELECT
      *
    FROM
      employees
    UNION
    ALL
    SELECT
      *
    FROM
      managers
    "###);

    assert_display_snapshot!(compile(r###"
    from employees
    select {name, cost = salary}
    take 3
    append (
        from employees
        select {name, cost = salary + bonuses}
        take 10
    )
    "###).unwrap(), @r###"
    WITH table_0 AS (
      SELECT
        name,
        salary + bonuses AS cost
      FROM
        employees
      LIMIT
        10
    )
    SELECT
      *
    FROM
      (
        SELECT
          name,
          salary AS cost
        FROM
          employees
        LIMIT
          3
      ) AS table_1
    UNION
    ALL
    SELECT
      *
    FROM
      table_0
    "###);

    assert_display_snapshot!(compile(r###"
    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))
    let union = `default_db.bottom` top -> (top | append bottom | distinct)

    from employees
    union managers
    "###).unwrap(), @r###"
    SELECT
      *
    FROM
      employees
    UNION
    DISTINCT
    SELECT
      *
    FROM
      managers
    "###);

    assert_display_snapshot!(compile(r###"
    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))
    let union = `default_db.bottom` top -> (top | append bottom | distinct)

    from employees
    append managers
    union all_employees_of_some_other_company
    "###).unwrap(), @r###"
    SELECT
      *
    FROM
      employees
    UNION
    ALL
    SELECT
      *
    FROM
      managers
    UNION
    DISTINCT
    SELECT
      *
    FROM
      all_employees_of_some_other_company
    "###);
}

#[test]
fn test_remove() {
    assert_display_snapshot!(compile(r#"
    from albums
    remove artists
    "#).unwrap(),
        @r###"
    SELECT
      *
    FROM
      albums AS t
    EXCEPT
      ALL
    SELECT
      *
    FROM
      artists AS b
    "###
    );

    assert_display_snapshot!(compile(r#"
    from album
    select artist_id
    remove (
        from artist | select artist_id
    )
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id
      FROM
        artist
    )
    SELECT
      artist_id
    FROM
      album
    EXCEPT
      ALL
    SELECT
      *
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    from album
    select {artist_id, title}
    remove (
        from artist | select artist_id
    )
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id
      FROM
        artist
    )
    SELECT
      album.artist_id,
      album.title
    FROM
      album
      LEFT JOIN table_0 ON album.artist_id = table_0.artist_id
    WHERE
      table_0.artist_id IS NULL
    "###
    );

    assert_display_snapshot!(compile(r#"
    prql target:sql.sqlite

    from album
    remove artist
    "#).unwrap_err(),
        @r###"
    Error: The dialect SQLiteDialect does not support EXCEPT ALL
    ↳ Hint: providing more column information will allow the query to be translated to an anti-join.
    "###
    );

    assert_display_snapshot!(compile(r#"
    prql target:sql.sqlite

    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))
    let except = `default_db.bottom` top -> (top | distinct | remove bottom)

    from album
    select {artist_id, title}
    except (from artist | select {artist_id, name})
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id,
        name
      FROM
        artist
    )
    SELECT
      artist_id,
      title
    FROM
      album
    EXCEPT
    SELECT
      *
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    prql target:sql.sqlite

    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))
    let except = `default_db.bottom` top -> (top | distinct | remove bottom)

    from album
    except artist
    "#).unwrap(),
        @r###"
    SELECT
      *
    FROM
      album AS t
    EXCEPT
    SELECT
      *
    FROM
      artist AS b
    "###
    );
}

#[test]
fn test_intersect() {
    assert_display_snapshot!(compile(r#"
    from album
    intersect artist
    "#).unwrap(),
        @r###"
    SELECT
      *
    FROM
      album AS t
    INTERSECT
    ALL
    SELECT
      *
    FROM
      artist AS b
    "###
    );

    assert_display_snapshot!(compile(r#"
    from album
    select artist_id
    intersect (
        from artist | select artist_id
    )
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id
      FROM
        artist
    )
    SELECT
      artist_id
    FROM
      album
    INTERSECT
    ALL
    SELECT
      *
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))

    from album
    select artist_id
    distinct
    intersect (
        from artist | select artist_id
    )
    distinct
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id
      FROM
        artist
    ),
    table_1 AS (
      SELECT
        artist_id
      FROM
        album
      INTERSECT
      DISTINCT
      SELECT
        *
      FROM
        table_0
    )
    SELECT
      DISTINCT artist_id
    FROM
      table_1
    "###
    );

    assert_display_snapshot!(compile(r#"
    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))

    from album
    select artist_id
    intersect (
        from artist | select artist_id
    )
    distinct
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id
      FROM
        artist
    ),
    table_1 AS (
      SELECT
        artist_id
      FROM
        album
      INTERSECT
      ALL
      SELECT
        *
      FROM
        table_0
    )
    SELECT
      DISTINCT artist_id
    FROM
      table_1
    "###
    );

    assert_display_snapshot!(compile(r#"
    let distinct = rel -> (from t = _param.rel | group {t.*} (take 1))

    from album
    select artist_id
    distinct
    intersect (
        from artist | select artist_id
    )
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        artist_id
      FROM
        artist
    )
    SELECT
      artist_id
    FROM
      album
    INTERSECT
    DISTINCT
    SELECT
      *
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    prql target:sql.sqlite

    from album
    intersect artist
    "#).unwrap_err(),
        @r###"
    Error: The dialect SQLiteDialect does not support INTERSECT ALL
    ↳ Hint: providing more column information will allow the query to be translated to an anti-join.
    "###
    );
}

#[test]
fn test_rn_ids_are_unique() {
    // this is wrong, output will have duplicate y_id and x_id
    assert_display_snapshot!((compile(r###"
    from y_orig
    group {y_id} (
        take 2 # take 1 uses `distinct` instead of partitioning, which might be a separate bug
    )
    group {x_id} (
        take 3
    )
    "###).unwrap()), @r###"
    WITH table_1 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (PARTITION BY y_id) AS _expr_1
      FROM
        y_orig
    ),
    table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (PARTITION BY x_id) AS _expr_0
      FROM
        table_1
      WHERE
        _expr_1 <= 2
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      _expr_0 <= 3
    "###);
}

#[test]
fn test_quoting() {
    // GH-#822
    assert_display_snapshot!((compile(r###"
    prql target:sql.postgres
    let UPPER = (
        default_db.lower
    )
    from UPPER
    join `some_schema.tablename` (==id)
    derive `from` = 5
    "###).unwrap()), @r###"
    WITH "UPPER" AS (
      SELECT
        *
      FROM
        lower
    )
    SELECT
      "UPPER".*,
      "some_schema.tablename".*,
      5 AS "from"
    FROM
      "UPPER"
      JOIN "some_schema.tablename" ON "UPPER".id = "some_schema.tablename".id
    "###);

    // GH-1493
    let query = r###"
    from `dir/*.parquet`
        # join files=`*.parquet` (==id)
    "###;
    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *
    FROM
      "dir/*.parquet"
    "###);

    // GH-#852
    assert_display_snapshot!((compile(r###"
    prql target:sql.bigquery
    from `db.schema.table`
    join `db.schema.table2` (==id)
    join c = `db.schema.t-able` (`db.schema.table`.id == c.id)
    "###).unwrap()), @r###"
    SELECT
      `db.schema.table`.*,
      `db.schema.table2`.*,
      c.*
    FROM
      `db.schema.table`
      JOIN `db.schema.table2` ON `db.schema.table`.id = `db.schema.table2`.id
      JOIN `db.schema.t-able` AS c ON `db.schema.table`.id = c.id
    "###);

    assert_display_snapshot!((compile(r###"
    default_db.table
    select `first name`
    "###).unwrap()), @r###"
    SELECT
      "first name"
    FROM
      "table"
    "###);

    assert_display_snapshot!((compile(r###"
        from as=Assessment
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      "Assessment" AS "as"
    "###);
}

#[test]
fn test_sorts_01() {
    assert_display_snapshot!((compile(r###"
    from invoices
    sort {issued_at, -amount, +num_of_articles}
    "###
    ).unwrap()), @r###"
    SELECT
      *
    FROM
      invoices
    ORDER BY
      issued_at,
      amount DESC,
      num_of_articles
    "###);

    assert_display_snapshot!((compile(r#"
    from x
    derive somefield = "something"
    sort {somefield}
    select {renamed = somefield}
    "#
    ).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        'something' AS renamed,
        'something' AS _expr_0
      FROM
        x
    )
    SELECT
      renamed
    FROM
      table_0
    ORDER BY
      _expr_0
    "###);
}

#[test]
fn test_sorts_02() {
    // issue #3129

    assert_display_snapshot!((compile(r###"
    let x = (
      from table
      sort index
      select {fieldA}
    )
    from x
    "###
    ).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        "fieldA",
        "index"
      FROM
        "table"
    ),
    x AS (
      SELECT
        "fieldA",
        "index"
      FROM
        table_0
    )
    SELECT
      "fieldA"
    FROM
      x
    ORDER BY
      "index"
    "###);

    // TODO: this is invalid SQL: a._expr_0 does not exist
    assert_display_snapshot!((compile(r#"
    from a
    join side:left b (==col)
    sort a.col
    select !{a.col}
    take 5
    "#
    ).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        a.*,
        b.*,
        a.col AS _expr_0
      FROM
        a
        LEFT JOIN b ON a.col = b.col
      ORDER BY
        a._expr_0
      LIMIT
        5
    )
    SELECT
      *
    FROM
      table_0
    ORDER BY
      _expr_0
    "###);
}

#[test]
fn test_numbers() {
    let query = r###"
    from numbers
    select {
        v = 5.000_000_1,
        w = 5_000,
        x = 5,
        y = 5.0,
        z = 5.00,
    }
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      5.0000001 AS v,
      5000 AS w,
      5 AS x,
      5.0 AS y,
      5.0 AS z
    FROM
      numbers
    "###);
}

#[test]
fn test_ranges() {
    assert_display_snapshot!((compile(r###"
    from employees
    derive {
      close = (distance | in ..50),
      middle = (distance | in 50..100),
      far = (distance | in 100..),
      country_founding | in @1776-07-04..@1787-09-17
    }
    "###).unwrap()), @r###"
    SELECT
      *,
      distance <= 50 AS close,
      distance BETWEEN 50 AND 100 AS middle,
      distance >= 100 AS far,
      country_founding BETWEEN DATE '1776-07-04' AND DATE '1787-09-17'
    FROM
      employees
    "###);
}

#[test]
fn test_in_values() {
    assert_display_snapshot!((compile(r#"
    from employees
    filter (title | in ["Sales Manager", "Sales Support Agent"])
    filter (employee_id | in [1, 2, 5])
    "#).unwrap()), @r#"
    SELECT
      *
    FROM
      employees
    WHERE
      title IN ('Sales Manager', 'Sales Support Agent')
      AND employee_id IN (1, 2, 5)
    "#);
}

#[test]
fn test_not_in_values() {
    assert_display_snapshot!((compile(r#"
    from employees
    filter !(title | in ["Sales Manager", "Sales Support Agent"])
    "#).unwrap()), @r#"
    SELECT
      *
    FROM
      employees
    WHERE
      NOT title IN ('Sales Manager', 'Sales Support Agent')
    "#);
}

#[test]
fn test_interval() {
    let query = r###"
    from projects
    derive first_check_in = start + 10days
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      start + INTERVAL 10 DAY AS first_check_in
    FROM
      projects
    "###);

    let query = r###"
    prql target:sql.postgres

    from projects
    derive first_check_in = start + 10days
    "###;
    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      start + INTERVAL '10' DAY AS first_check_in
    FROM
      projects
    "###);
}

#[test]
fn test_dates() {
    assert_display_snapshot!((compile(r###"
    from to_do_empty_table
    derive {
        date = @2011-02-01,
        timestamp = @2011-02-01T10:00,
        time = @14:00,
        # datetime = @2011-02-01T10:00<datetime>,
    }
    "###).unwrap()), @r###"
    SELECT
      *,
      DATE '2011-02-01' AS date,
      TIMESTAMP '2011-02-01T10:00' AS timestamp,
      TIME '14:00' AS time
    FROM
      to_do_empty_table
    "###);
}

#[test]
fn test_window_functions_00() {
    assert_display_snapshot!((compile(r###"
    from employees
    group last_name (
        derive {count first_name}
    )
    "###).unwrap()), @r###"
    SELECT
      *,
      COUNT(*) OVER (PARTITION BY last_name)
    FROM
      employees
    "###);
}

#[test]
fn test_window_functions_02() {
    let query = r#"
    from co=cust_order
    join ol=order_line (==order_id)
    derive {
        order_month = s"TO_CHAR({co.order_date}, '%Y-%m')",
        order_day = s"TO_CHAR({co.order_date}, '%Y-%m-%d')",
    }
    group {order_month, order_day} (
        aggregate {
            num_orders = s"COUNT(DISTINCT {co.order_id})",
            num_books = count ol.book_id,
            total_price = sum ol.price,
        }
    )
    group {order_month} (
        sort order_day
        window expanding:true (
            derive {running_total_num_books = sum num_books}
        )
    )
    sort order_day
    derive {num_books_last_week = lag 7 num_books}
    "#;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        TO_CHAR(co.order_date, '%Y-%m') AS order_month,
        TO_CHAR(co.order_date, '%Y-%m-%d') AS order_day,
        COUNT(DISTINCT co.order_id) AS num_orders,
        COUNT(*) AS num_books,
        COALESCE(SUM(ol.price), 0) AS total_price
      FROM
        cust_order AS co
        JOIN order_line AS ol ON co.order_id = ol.order_id
      GROUP BY
        TO_CHAR(co.order_date, '%Y-%m'),
        TO_CHAR(co.order_date, '%Y-%m-%d')
    )
    SELECT
      order_month,
      order_day,
      num_orders,
      num_books,
      total_price,
      SUM(num_books) OVER (
        PARTITION BY order_month
        ORDER BY
          order_day ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
      ) AS running_total_num_books,
      LAG(num_books, 7) OVER (
        ORDER BY
          order_day
      ) AS num_books_last_week
    FROM
      table_0
    ORDER BY
      order_day
    "###);
}

#[test]
fn test_window_functions_03() {
    // lag must be recognized as window function, even outside of group context
    // rank must not have two OVER clauses
    let query = r###"
    from daily_orders
    derive {last_week = lag 7 num_orders}
    derive {first_count = first num_orders}
    derive {last_count = last num_orders}
    group month (
      derive {total_month = sum num_orders}
    )
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      LAG(num_orders, 7) OVER () AS last_week,
      FIRST_VALUE(num_orders) OVER () AS first_count,
      LAST_VALUE(num_orders) OVER () AS last_count,
      SUM(num_orders) OVER (PARTITION BY month) AS total_month
    FROM
      daily_orders
    "###);
}

#[test]
fn test_window_functions_04() {
    // sort does not affects into groups, group undoes sorting
    let query = r###"
    from daily_orders
    sort day
    group month (derive {total_month = rank day})
    derive {last_week = lag 7 num_orders}
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      RANK() OVER (PARTITION BY month) AS total_month,
      LAG(num_orders, 7) OVER () AS last_week
    FROM
      daily_orders
    "###);
}

#[test]
fn test_window_functions_05() {
    // sort does not leak out of groups
    let query = r###"
    from daily_orders
    sort day
    group month (sort num_orders | window expanding:true (derive {rank day}))
    derive {num_orders_last_week = lag 7 num_orders}
    "###;
    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      RANK() OVER (
        PARTITION BY month
        ORDER BY
          num_orders
      ),
      LAG(num_orders, 7) OVER () AS num_orders_last_week
    FROM
      daily_orders
    "###);
}

#[test]
fn test_window_functions_06() {
    // detect sum as a window function, even without group or window
    assert_display_snapshot!((compile(r###"
    from foo
    derive {a = sum b}
    group c (
        derive {d = sum b}
    )
    "###).unwrap()), @r###"
    SELECT
      *,
      SUM(b) OVER () AS a,
      SUM(b) OVER (PARTITION BY c) AS d
    FROM
      foo
    "###);
}

#[test]
fn test_window_functions_07() {
    assert_display_snapshot!((compile(r###"
    from foo
    window expanding:true (
        derive {running_total = sum b}
    )
    "###).unwrap()), @r###"
    SELECT
      *,
      SUM(b) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS running_total
    FROM
      foo
    "###);
}

#[test]
fn test_window_functions_08() {
    assert_display_snapshot!((compile(r###"
    from foo
    window rolling:3 (
        derive {last_three = sum b}
    )
    "###).unwrap()), @r###"
    SELECT
      *,
      SUM(b) OVER (ROWS BETWEEN 2 PRECEDING AND CURRENT ROW) AS last_three
    FROM
      foo
    "###);
}

#[test]
fn test_window_functions_09() {
    assert_display_snapshot!((compile(r###"
    from foo
    window rows:0..4 (
        derive {next_four_rows = sum b}
    )
    "###).unwrap()), @r###"
    SELECT
      *,
      SUM(b) OVER (
        ROWS BETWEEN CURRENT ROW
        AND 4 FOLLOWING
      ) AS next_four_rows
    FROM
      foo
    "###);
}

#[test]
fn test_window_functions_10() {
    assert_display_snapshot!((compile(r###"
    from foo
    sort day
    window range:-4..4 (
        derive {next_four_days = sum b}
    )
    "###).unwrap()), @r###"
    SELECT
      *,
      SUM(b) OVER (
        ORDER BY
          day RANGE BETWEEN 4 PRECEDING AND 4 FOLLOWING
      ) AS next_four_days
    FROM
      foo
    ORDER BY
      day
    "###);
}

#[test]
fn test_window_functions_11() {
    assert_display_snapshot!((compile(r###"
    from employees
    sort age
    derive {num = row_number this}
    "###).unwrap()), @r###"
    SELECT
      *,
      ROW_NUMBER() OVER (
        ORDER BY
          age
      ) AS num
    FROM
      employees
    ORDER BY
      age
    "###);
}

#[test]
fn test_window_functions_12() {
    // window params need to be simple expressions

    assert_display_snapshot!((compile(r###"
    from x
    derive {b = lag 1 a}
    window (
      sort b
      derive {c = lag 1 a}
    )
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        LAG(a, 1) OVER () AS b
      FROM
        x
    )
    SELECT
      *,
      LAG(a, 1) OVER (
        ORDER BY
          b
      ) AS c
    FROM
      table_0
    ORDER BY
      b
    "###);

    assert_display_snapshot!((compile(r###"
    from x
    derive {b = lag 1 a}
    group b (
      derive {c = lag 1 a}
    )
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        LAG(a, 1) OVER () AS b,
        *
      FROM
        x
    )
    SELECT
      *,
      LAG(a, 1) OVER (PARTITION BY b) AS c
    FROM
      table_0
    "###);
}

#[test]
fn test_window_functions_13() {
    // window params need to be simple expressions

    assert_display_snapshot!((compile(r###"
    from tracks
    group {album_id} (
      window (derive {grp = milliseconds - (row_number this)})
    )
    group {grp} (
      window (derive {count = row_number this})
    )
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (PARTITION BY album_id) AS _expr_0
      FROM
        tracks
    )
    SELECT
      milliseconds - _expr_0 AS grp,
      *,
      ROW_NUMBER() OVER (PARTITION BY milliseconds - _expr_0) AS count
    FROM
      table_0
    "###);
}

#[test]
fn test_name_resolving() {
    let query = r###"
    from numbers
    derive x = 5
    select {y = 6, z = x + y + a}
    "###;
    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      6 AS y,
      5 + 6 + a AS z
    FROM
      numbers
    "###);
}

#[test]
fn test_strings() {
    let query = r#"
    from empty_table_to_do
    select {
        x = "two households'",
        y = 'two households"',
        z = f"a {x} b' {y} c",
        v = f'a {x} b" {y} c',
    }
    "#;
    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      'two households''' AS x,
      'two households"' AS y,
      CONCAT(
        'a ',
        'two households''',
        ' b'' ',
        'two households"',
        ' c'
      ) AS z,
      CONCAT(
        'a ',
        'two households''',
        ' b" ',
        'two households"',
        ' c'
      ) AS v
    FROM
      empty_table_to_do
    "###);
}

#[test]
fn test_filter() {
    // https://github.com/PRQL/prql/issues/469
    let query = r###"
    from employees
    filter {age > 25, age < 40}
    "###;

    assert!(compile(query).is_err());

    assert_display_snapshot!((compile(r###"
    from employees
    filter age > 25 && age < 40
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    WHERE
      age > 25
      AND age < 40
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    filter age > 25
    filter age < 40
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    WHERE
      age > 25
      AND age < 40
    "###);
}

#[test]
fn test_nulls() {
    assert_display_snapshot!((compile(r###"
    from employees
    select amount = null
    "###).unwrap()), @r###"
    SELECT
      NULL AS amount
    FROM
      employees
    "###);

    // coalesce
    assert_display_snapshot!((compile(r###"
    from employees
    derive amount = amount + 2 ?? 3 * 5
    "###).unwrap()), @r###"
    SELECT
      *,
      COALESCE(amount + 2, 3 * 5) AS amount
    FROM
      employees
    "###);

    // IS NULL
    assert_display_snapshot!((compile(r###"
    from employees
    filter first_name == null && null == last_name
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    WHERE
      first_name IS NULL
      AND last_name IS NULL
    "###);

    // IS NOT NULL
    assert_display_snapshot!((compile(r###"
    from employees
    filter first_name != null && null != last_name
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    WHERE
      first_name IS NOT NULL
      AND last_name IS NOT NULL
    "###);
}

#[test]
fn test_take() {
    assert_display_snapshot!((compile(r###"
    from employees
    take ..10
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    LIMIT
      10
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    take 5..10
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    LIMIT
      6 OFFSET 4
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    take 5..
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees OFFSET 4
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    take 5..5
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    LIMIT
      1 OFFSET 4
    "###);

    // should be one SELECT
    assert_display_snapshot!((compile(r###"
    from employees
    take 11..20
    take 1..5
    "###).unwrap()), @r###"
    SELECT
      *
    FROM
      employees
    LIMIT
      5 OFFSET 10
    "###);

    // should be two SELECTs
    assert_display_snapshot!((compile(r###"
    from employees
    take 11..20
    sort name
    take 1..5
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        employees
      LIMIT
        10 OFFSET 10
    )
    SELECT
      *
    FROM
      table_0
    ORDER BY
      name
    LIMIT
      5
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    take 0..1
    "###).unwrap_err()), @r###"
    Error:
       ╭─[:3:5]
       │
     3 │     take 0..1
       │     ────┬────
       │         ╰────── take expected a positive int range, but found 0..1
    ───╯
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    take (-1..)
    "###).unwrap_err()), @r###"
    Error:
       ╭─[:3:5]
       │
     3 │     take (-1..)
       │     ─────┬─────
       │          ╰─────── take expected a positive int range, but found -1..
    ───╯
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    select a
    take 5..5.6
    "###).unwrap_err()), @r###"
    Error:
       ╭─[:4:5]
       │
     4 │     take 5..5.6
       │     ─────┬─────
       │          ╰─────── take expected a positive int range, but found 5..?
    ───╯
    "###);

    assert_display_snapshot!((compile(r###"
    from employees
    take (-1)
    "###).unwrap_err()), @r###"
    Error:
       ╭─[:3:5]
       │
     3 │     take (-1)
       │     ────┬────
       │         ╰────── take expected a positive int range, but found ..-1
    ───╯
    "###);
}

#[test]
fn test_distinct_01() {
    // window functions cannot materialize into where statement: CTE is needed
    assert_display_snapshot!((compile(r###"
    from employees
    derive {rn = row_number id}
    filter rn > 2
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER () AS rn
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      rn > 2
    "###);
}

#[test]
fn test_distinct_02() {
    // basic distinct
    assert_display_snapshot!((compile(r###"
    from employees
    select first_name
    group first_name (take 1)
    "###).unwrap()), @r###"
    SELECT
      DISTINCT first_name
    FROM
      employees
    "###);
}

#[test]
fn test_distinct_03() {
    // distinct on two columns
    assert_display_snapshot!((compile(r###"
    from employees
    select {first_name, last_name}
    group {first_name, last_name} (take 1)
    "###).unwrap()), @r###"
    SELECT
      DISTINCT first_name,
      last_name
    FROM
      employees
    "###);
}
#[test]
fn test_distinct_04() {
    // We want distinct only over first_name and last_name, so we can't use a
    // `DISTINCT *` here.
    assert_display_snapshot!((compile(r###"
    from employees
    group {first_name, last_name} (take 1)
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (PARTITION BY first_name, last_name) AS _expr_0
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      _expr_0 <= 1
    "###);
}
#[test]
fn test_distinct_05() {
    // Check that a different order doesn't stop distinct from being used.
    assert!(compile(
        "from employees | select {first_name, last_name} | group {last_name, first_name} (take 1)"
    )
    .unwrap()
    .contains("DISTINCT"));
}
#[test]
fn test_distinct_06() {
    // head
    assert_display_snapshot!((compile(r###"
    from employees
    group department (take 3)
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (PARTITION BY department) AS _expr_0
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      _expr_0 <= 3
    "###);
}
#[test]
fn test_distinct_07() {
    assert_display_snapshot!((compile(r###"
    from employees
    group department (sort salary | take 2..3)
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (
          PARTITION BY department
          ORDER BY
            salary
        ) AS _expr_0
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      _expr_0 BETWEEN 2 AND 3
    "###);
}
#[test]
fn test_distinct_08() {
    assert_display_snapshot!((compile(r###"
    from employees
    group department (sort salary | take 4..4)
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        *,
        ROW_NUMBER() OVER (
          PARTITION BY department
          ORDER BY
            salary
        ) AS _expr_0
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      _expr_0 = 4
    "###);
}

#[test]
fn test_distinct_09() {
    assert_display_snapshot!(compile("
    from invoices
    select {billing_country, billing_city}
    group {billing_city} (
      take 1
    )
    sort billing_city
    ").unwrap(), @r###"
    WITH table_0 AS (
      SELECT
        billing_city,
        billing_country,
        ROW_NUMBER() OVER (PARTITION BY billing_city) AS _expr_0
      FROM
        invoices
    )
    SELECT
      billing_city,
      billing_country
    FROM
      table_0
    WHERE
      _expr_0 <= 1
    ORDER BY
      billing_city
    "###);
}

#[test]
fn test_distinct_on_01() {
    assert_display_snapshot!((compile(r###"
    prql target:sql.postgres

    from employees
    group department (
      sort age
      take 1
    )
    "###).unwrap()), @r###"
    SELECT
      DISTINCT ON (department) *
    FROM
      employees
    ORDER BY
      department,
      age
    "###);
}

#[test]
fn test_distinct_on_02() {
    assert_display_snapshot!((compile(r###"
    prql target:sql.duckdb

    from x
    select {class, begins}
    group {begins} (take 1)
    "###).unwrap()), @r###"
    SELECT
      DISTINCT ON (begins) begins,
      class
    FROM
      x
    "###);
}

#[test]
fn test_distinct_on_03() {
    assert_display_snapshot!((compile(r###"
    prql target:sql.duckdb

    from tab1
    group col1 (
      take 1
    )
    derive foo = 1
    select foo
    "###).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        DISTINCT ON (col1) NULL
      FROM
        tab1
    )
    SELECT
      1 AS foo
    FROM
      table_0
    "###);
}

#[test]
fn test_distinct_on_04() {
    assert_display_snapshot!((compile(r###"
    prql target:sql.duckdb

    from a
    join b (b.a_id == a.id)
    group {a.id} (
      sort b.x
      take 1
    )
    select {a.id, b.y}
    "###).unwrap()), @r###"
    SELECT
      DISTINCT ON (a.id) a.id,
      b.y
    FROM
      a
      JOIN b ON b.a_id = a.id
    ORDER BY
      a.id,
      b.x
    "###);
}

#[test]
fn test_join() {
    assert_display_snapshot!((compile(r###"
    from x
    join y (==id)
    "###).unwrap()), @r###"
    SELECT
      x.*,
      y.*
    FROM
      x
      JOIN y ON x.id = y.id
    "###);

    compile("from x | join y {==x.id}").unwrap_err();
}

#[test]
fn test_from_json() {
    // Test that the SQL generated from the JSON of the PRQL is the same as the raw PRQL
    let original_prql = r#"
    from e=employees
    join salaries (==emp_no)
    group {e.emp_no, e.gender} (
      aggregate {
        emp_salary = average salaries.salary
      }
    )
    join de=dept_emp (==emp_no)
    join dm=dept_manager (
      (dm.dept_no == de.dept_no) && s"(de.from_date, de.to_date) OVERLAPS (dm.from_date, dm.to_date)"
    )
    group {dm.emp_no, gender} (
      aggregate {
        salary_avg = average emp_salary,
        salary_sd = stddev emp_salary
      }
    )
    derive mng_no = emp_no
    join managers=employees (==emp_no)
    derive mng_name = s"managers.first_name || ' ' || managers.last_name"
    select {mng_name, managers.gender, salary_avg, salary_sd}
    "#;

    let mut source_tree = SourceTree::from(original_prql);
    prql_compiler::semantic::load_std_lib(&mut source_tree);

    let sql_from_prql = Ok(prql_compiler::prql_to_pl_tree(&source_tree).unwrap())
        .and_then(|ast| prql_compiler::semantic::resolve_and_lower(ast, &[]))
        .and_then(|rq| sql::compile(rq, &Options::default()))
        .unwrap();

    let sql_from_json = prql_compiler::prql_to_pl(original_prql)
        .and_then(prql_compiler::json::from_pl)
        .and_then(|json| prql_compiler::json::to_pl(&json))
        .and_then(prql_compiler::pl_to_rq)
        .and_then(|rq| prql_compiler::rq_to_sql(rq, &Options::default()))
        .unwrap();

    assert_eq!(sql_from_prql, sql_from_json);
}

#[test]
fn test_f_string() {
    let query = r#"
    from employees
    derive age = year_born - s'now()'
    select {
        f"Hello my name is {prefix}{first_name} {last_name}",
        f"and I am {age} years old."
    }
    "#;

    assert_display_snapshot!(
      compile(query).unwrap(),
        @r###"
    SELECT
      CONCAT(
        'Hello my name is ',
        prefix,
        first_name,
        ' ',
        last_name
      ),
      CONCAT('and I am ', year_born - now(), ' years old.')
    FROM
      employees
    "###
    );

    assert_display_snapshot!(
        prql_compiler::compile(
          query,
          &Options::default()
              .no_signature()
              .with_target(Target::Sql(Some(sql::Dialect::SQLite)))

      ).unwrap(),
          @r###"
    SELECT
      'Hello my name is ' || prefix || first_name || ' ' || last_name,
      'and I am ' || year_born - now() || ' years old.'
    FROM
      employees
    "###
    );
}

#[test]
fn test_sql_of_ast_1() {
    let query = r#"
    from employees
    filter country == "USA"
    group {title, country} (
        aggregate {average salary}
    )
    sort title
    take 20
    "#;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    SELECT
      title,
      country,
      AVG(salary)
    FROM
      employees
    WHERE
      country = 'USA'
    GROUP BY
      title,
      country
    ORDER BY
      title
    LIMIT
      20
    "###
    );
}

#[test]
// Confirm that a bare s-string in a table definition works as expected.
fn test_bare_s_string() {
    let query = r#"
    let grouping = s"""
        SELECT SUM(a)
        FROM tbl
        GROUP BY
          GROUPING SETS
          ((b, c, d), (d), (b, d))
      """
    from grouping
    "#;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    WITH table_0 AS (
      SELECT
        SUM(a)
      FROM
        tbl
      GROUP BY
        GROUPING SETS ((b, c, d), (d), (b, d))
    )
    SELECT
      *
    FROM
      table_0
    "###
    );

    // Test that case insensitive SELECT is accepted. We allow it as it is valid SQL.
    let query = r#"
    let a = s"select insensitive from rude"
    from a
    "#;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    WITH table_0 AS (
      SELECT
        insensitive
      from
        rude
    )
    SELECT
      *
    FROM
      table_0
    "###
    );

    // Check a mixture of cases for good measure.
    let query = r#"
    let a = s"sElEcT insensitive from rude"
    from a
    "#;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    WITH table_0 AS (
      SELECT
        insensitive
      from
        rude
    )
    SELECT
      *
    FROM
      table_0
    "###
    );

    // Check SELECT\n.
    let query = r#"
    let a = s"
    SELECT
      foo
    FROM
      bar"

    from a
    "#;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
      @r###"
    WITH table_0 AS (
      SELECT
        foo
      FROM
        bar
    )
    SELECT
      *
    FROM
      table_0
    "###);

    assert_display_snapshot!(compile(r#"
    from s"SELECTfoo"
    "#).unwrap_err(), @r###"
    Error: s-strings representing a table must start with `SELECT `
    ↳ Hint: this is a limitation by current compiler implementation
    "###);
}

#[test]
// Confirm that a regular expr_call in a table definition works as expected.
fn test_table_definition_with_expr_call() {
    let query = r###"
    let e = take 4 (from employees)
    from e
    "###;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    WITH e AS (
      SELECT
        *
      FROM
        employees
      LIMIT
        4
    )
    SELECT
      *
    FROM
      e
    "###
    );
}

#[test]
fn test_sql_of_ast_2() {
    let query = r#"
    from employees
    aggregate sum_salary = s"sum({salary})"
    filter sum_salary > 100
    "#;
    let sql = compile(query).unwrap();
    assert_snapshot!(sql, @r###"
    SELECT
      sum(salary) AS sum_salary
    FROM
      employees
    HAVING
      sum(salary) > 100
    "###);
    assert!(sql.to_lowercase().contains(&"having".to_lowercase()));
}

#[test]
fn test_prql_to_sql_1() {
    assert_display_snapshot!(compile(r#"
    from employees
    aggregate {
        count salary,
        sum salary,
    }
    "#).unwrap(),
        @r###"
    SELECT
      COUNT(*),
      COALESCE(SUM(salary), 0)
    FROM
      employees
    "###
    );
    assert_display_snapshot!(compile(r#"
    prql target:sql.postgres
    from developers
    group team (
        aggregate {
            skill_width = count_distinct specialty,
        }
    )
    "#).unwrap(),
        @r###"
    SELECT
      team,
      COUNT(DISTINCT specialty) AS skill_width
    FROM
      developers
    GROUP BY
      team
    "###
    )
}

#[test]
fn test_prql_to_sql_2() {
    let query = r#"
from employees
filter country == "USA"                           # Each line transforms the previous result.
derive {                                         # This adds columns / variables.
gross_salary = salary + payroll_tax,
gross_cost = gross_salary + benefits_cost      # Variables can use other variables.
}
filter gross_cost > 0
group {title, country} (
aggregate  {                                 # `by` are the columns to group by.
    average salary,                          # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,
    ct = count salary,
}
)
sort sum_gross_cost
filter ct > 200
take 20
"#;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql, @r###"
    WITH table_0 AS (
      SELECT
        title,
        country,
        salary + payroll_tax + benefits_cost AS _expr_0,
        salary + payroll_tax AS _expr_1,
        salary
      FROM
        employees
      WHERE
        country = 'USA'
    )
    SELECT
      title,
      country,
      AVG(salary),
      COALESCE(SUM(salary), 0),
      AVG(_expr_1),
      COALESCE(SUM(_expr_1), 0),
      AVG(_expr_0),
      COALESCE(SUM(_expr_0), 0) AS sum_gross_cost,
      COUNT(*) AS ct
    FROM
      table_0
    WHERE
      _expr_0 > 0
    GROUP BY
      title,
      country
    HAVING
      COUNT(*) > 200
    ORDER BY
      sum_gross_cost
    LIMIT
      20
    "###);
}

#[test]
fn test_prql_to_sql_table() {
    // table
    let query = r#"
    let newest_employees = (
        from employees
        sort tenure
        take 50
    )
    let average_salaries = (
        from salaries
        group country (
            aggregate {
                average_country_salary = average salary
            }
        )
    )
    from newest_employees
    join average_salaries (==country)
    select {name, salary, average_country_salary}
    "#;
    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    WITH newest_employees AS (
      SELECT
        *
      FROM
        employees
      ORDER BY
        tenure
      LIMIT
        50
    ), average_salaries AS (
      SELECT
        country,
        AVG(salary) AS average_country_salary
      FROM
        salaries
      GROUP BY
        country
    )
    SELECT
      newest_employees.name,
      newest_employees.salary,
      average_salaries.average_country_salary
    FROM
      newest_employees
      JOIN average_salaries ON newest_employees.country = average_salaries.country
    ORDER BY
      employees.tenure
    "###
    );
}

#[test]
fn test_nonatomic() {
    // A take, then two aggregates
    let query = r#"
        from employees
        take 20
        filter country == "USA"
        group {title, country} (
            aggregate {
                salary = average salary
            }
        )
        group {title, country} (
            aggregate {
                sum_gross_cost = average salary
            }
        )
        sort sum_gross_cost
    "#;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    WITH table_1 AS (
      SELECT
        title,
        country,
        salary
      FROM
        employees
      LIMIT
        20
    ), table_0 AS (
      SELECT
        title,
        country,
        AVG(salary) AS _expr_0
      FROM
        table_1
      WHERE
        country = 'USA'
      GROUP BY
        title,
        country
    )
    SELECT
      title,
      country,
      AVG(_expr_0) AS sum_gross_cost
    FROM
      table_0
    GROUP BY
      title,
      country
    ORDER BY
      sum_gross_cost
    "###);

    // A aggregate, then sort and filter
    let query = r###"
        from employees
        group {title, country} (
            aggregate {
                sum_gross_cost = average salary
            }
        )
        sort sum_gross_cost
        filter sum_gross_cost > 0
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      title,
      country,
      AVG(salary) AS sum_gross_cost
    FROM
      employees
    GROUP BY
      title,
      country
    HAVING
      AVG(salary) > 0
    ORDER BY
      sum_gross_cost
    "###);
}

#[test]
/// Confirm a nonatomic table works.
fn test_nonatomic_table() {
    // A take, then two aggregates
    let query = r#"
    let a = (
        from employees
        take 50
        group country (aggregate {s"count(*)"})
    )
    from a
    join b (==country)
    select {name, salary, average_country_salary}
"#;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    WITH table_0 AS (
      SELECT
        country
      FROM
        employees
      LIMIT
        50
    ), a AS (
      SELECT
        country,
        count(*)
      FROM
        table_0
      GROUP BY
        country
    )
    SELECT
      b.name,
      b.salary,
      b.average_country_salary
    FROM
      a
      JOIN b ON a.country = b.country
    "###);
}

#[test]
fn test_table_names_between_splits() {
    let prql = r###"
    from employees
    join d=department (==dept_no)
    take 10
    derive emp_no = employees.emp_no
    join s=salaries (==emp_no)
    select {employees.emp_no, d.name, s.salary}
    "###;
    let result = compile(prql).unwrap();
    assert_display_snapshot!(result, @r###"
    WITH table_0 AS (
      SELECT
        employees.emp_no,
        d.name
      FROM
        employees
        JOIN department AS d ON employees.dept_no = d.dept_no
      LIMIT
        10
    )
    SELECT
      table_0.emp_no,
      table_0.name,
      s.salary
    FROM
      table_0
      JOIN salaries AS s ON table_0.emp_no = s.emp_no
    "###);

    let prql = r###"
    from e=employees
    take 10
    join salaries (==emp_no)
    select {e.*, salaries.salary}
    "###;
    let result = compile(prql).unwrap();
    assert_display_snapshot!(result, @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        employees AS e
      LIMIT
        10
    )
    SELECT
      table_0.*,
      salaries.salary
    FROM
      table_0
      JOIN salaries ON table_0.emp_no = salaries.emp_no
    "###);
}

#[test]
fn test_table_alias() {
    // Alias on from
    let query = r###"
        from e = employees
        join salaries side:left (salaries.emp_no == e.emp_no)
        group {e.emp_no} (
            aggregate {
                emp_salary = average salaries.salary
            }
        )
        select {emp_no, emp_salary}
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      e.emp_no,
      AVG(salaries.salary) AS emp_salary
    FROM
      employees AS e
      LEFT JOIN salaries ON salaries.emp_no = e.emp_no
    GROUP BY
      e.emp_no
    "###);

    assert_display_snapshot!((compile(r#"
    from e=employees
    select e.first_name
    filter e.first_name == "Fred"
    "#).unwrap()), @r###"
    SELECT
      first_name
    FROM
      employees AS e
    WHERE
      first_name = 'Fred'
    "###);
}

#[test]
fn test_targets() {
    // Generic
    let query = r###"
    prql target:sql.generic
    from Employees
    select {FirstName, `last name`}
    take 3
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      "FirstName",
      "last name"
    FROM
      "Employees"
    LIMIT
      3
    "###);

    // SQL server
    let query = r###"
    prql target:sql.mssql
    from Employees
    select {FirstName, `last name`}
    take 3
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      TOP (3) "FirstName",
      "last name"
    FROM
      "Employees"
    "###);

    // MySQL
    let query = r###"
    prql target:sql.mysql
    from Employees
    select {FirstName, `last name`}
    take 3
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      `FirstName`,
      `last name`
    FROM
      `Employees`
    LIMIT
      3
    "###);
}

#[test]
fn test_target_clickhouse() {
    let query = r###"
    prql target:sql.clickhouse

    from github_json
    derive {event_type_dotted = `event.type`}
    "###;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      `event.type` AS event_type_dotted
    FROM
      github_json
    "###);
}

#[test]
fn test_ident_escaping() {
    // Generic
    let query = r#"
    from `anim"ls`
    derive {`čebela` = BeeName, medved = `bear's_name`}
    "#;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      "BeeName" AS "čebela",
      "bear's_name" AS medved
    FROM
      "anim""ls"
    "###);

    // MySQL
    let query = r#"
    prql target:sql.mysql

    from `anim"ls`
    derive {`čebela` = BeeName, medved = `bear's_name`}
    "#;

    assert_display_snapshot!((compile(query).unwrap()), @r###"
    SELECT
      *,
      `BeeName` AS `čebela`,
      `bear's_name` AS medved
    FROM
      `anim"ls`
    "###);
}

#[test]
fn test_literal() {
    let query = r###"
    from employees
    derive {always_true = true}
    "###;

    let sql = compile(query).unwrap();
    assert_display_snapshot!(sql,
        @r###"
    SELECT
      *,
      true AS always_true
    FROM
      employees
    "###
    );
}

#[test]
fn test_same_column_names() {
    // #820
    let query = r###"
let x = (
from x_table
select only_in_x = foo
)

let y = (
from y_table
select foo
)

from x
join y (foo == only_in_x)
"###;

    assert_display_snapshot!(compile(query).unwrap(),
        @r###"
    WITH x AS (
      SELECT
        foo AS only_in_x
      FROM
        x_table
    ),
    y AS (
      SELECT
        foo
      FROM
        y_table
    )
    SELECT
      x.only_in_x,
      y.foo
    FROM
      x
      JOIN y ON y.foo = x.only_in_x
    "###
    );
}

#[test]
fn test_double_aggregate() {
    // #941
    compile(
        r###"
    from numbers
    group {type} (
        aggregate {
            total_amt = sum amount,
        }
        aggregate {
            max amount
        }
    )
    "###,
    )
    .unwrap_err();

    assert_display_snapshot!(compile(r###"
    from numbers
    group {`type`} (
        aggregate {
            total_amt = sum amount,
            max amount
        }
    )
    "###).unwrap(),
        @r###"
    SELECT
      type,
      COALESCE(SUM(amount), 0) AS total_amt,
      MAX(amount)
    FROM
      numbers
    GROUP BY
      type
    "###
    );
}

#[test]
fn test_window_function_coalesce() {
    // #3587
    assert_display_snapshot!(compile(r###"
    from x
    select {a, b=a}
    window (
      select {
        cumsum_a=(sum a),
        cumsum_b=(sum b)
      }
    )
    "###).unwrap(),
        @r###"
    SELECT
      SUM(a) OVER () AS cumsum_a,
      SUM(a) OVER () AS cumsum_b
    FROM
      x
    "###
    );
}

#[test]
fn test_casting() {
    assert_display_snapshot!(compile(r###"
    from x
    select {a}
    derive {
        b = (a | as int) + 10,
        c = (a | as int) - 10,
        d = (a | as float) * 10,
        e = (a | as float) / 10,
    }
    "###).unwrap(),
        @r###"
    SELECT
      a,
      CAST(a AS int) + 10 AS b,
      CAST(a AS int) - 10 AS c,
      CAST(a AS float) * 10 AS d,
      CAST(a AS float) / 10 AS e
    FROM
      x
    "###
    );
}

#[test]
fn test_toposort() {
    // #1183

    assert_display_snapshot!(compile(r###"
    let b = (
        from somesource
    )

    let a = (
        from b
    )

    from b
    "###).unwrap(),
        @r###"
    WITH b AS (
      SELECT
        *
      FROM
        somesource
    )
    SELECT
      *
    FROM
      b
    "###
    );
}

#[test]
fn test_inline_tables() {
    assert_display_snapshot!(compile(r###"
    (
        from employees
        select {emp_id, name, surname, `type`, amount}
    )
    join s = (from salaries | select {emp_id, salary}) (==emp_id)
    "###).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        emp_id,
        salary
      FROM
        salaries
    )
    SELECT
      employees.emp_id,
      employees.name,
      employees.surname,
      employees.type,
      employees.amount,
      table_0.emp_id,
      table_0.salary
    FROM
      employees
      JOIN table_0 ON employees.emp_id = table_0.emp_id
    "###
    );
}

#[test]
fn test_filter_and_select_unchanged_alias() {
    // #1185

    assert_display_snapshot!(compile(r###"
    from account
    filter account.name != null
    select {name = account.name}
    "###).unwrap(),
        @r###"
    SELECT
      name
    FROM
      account
    WHERE
      name IS NOT NULL
    "###
    );
}

#[test]
fn test_filter_and_select_changed_alias() {
    // #1185
    assert_display_snapshot!(compile(r###"
    from account
    filter account.name != null
    select {renamed_name = account.name}
    "###).unwrap(),
        @r###"
    SELECT
      name AS renamed_name
    FROM
      account
    WHERE
      name IS NOT NULL
    "###
    );

    // #1207
    assert_display_snapshot!(compile(r#"
    from x
    filter name != "Bob"
    select name = name ?? "Default"
    "#).unwrap(),
        @r###"
    SELECT
      COALESCE(name, 'Default') AS name
    FROM
      x
    WHERE
      name <> 'Bob'
    "###
    );
}

#[test]
fn test_unused_alias() {
    // #1308
    assert_display_snapshot!(compile(r###"
    from account
    select n = {account.name}
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:16]
       │
     3 │     select n = {account.name}
       │                ───────┬──────
       │                       ╰──────── unexpected assign to `n`
       │
       │ Help: move assign into the tuple: `[n = ...]`
    ───╯
    "###)
}

#[test]
fn test_table_s_string() {
    assert_display_snapshot!(compile(r#"
    let main <relation> = s"SELECT DISTINCT ON first_name, age FROM employees ORDER BY age ASC"
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        DISTINCT ON first_name,
        age
      FROM
        employees
      ORDER BY
        age ASC
    )
    SELECT
      *
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    from s"""
        SELECT DISTINCT ON first_name, id, age FROM employees ORDER BY age ASC
    """
    join s = s"SELECT * FROM salaries" (==id)
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        DISTINCT ON first_name,
        id,
        age
      FROM
        employees
      ORDER BY
        age ASC
    ),
    table_1 AS (
      SELECT
        *
      FROM
        salaries
    )
    SELECT
      table_0.*,
      table_1.*
    FROM
      table_0
      JOIN table_1 ON table_0.id = table_1.id
    "###
    );

    assert_display_snapshot!(compile(r#"
    from s"""SELECT * FROM employees"""
    filter country == "USA"
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      country = 'USA'
    "###
    );

    assert_display_snapshot!(compile(r#"
    from e=s"""SELECT * FROM employees"""
    filter e.country == "USA"
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        employees
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      country = 'USA'
    "###
    );

    assert_display_snapshot!(compile(r#"
    let weeks_between = start end -> s"SELECT generate_series({start}, {end}, '1 week') as date"
    let current_week = -> s"date(date_trunc('week', current_date))"

    weeks_between @2022-06-03 (current_week + 4)
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        generate_series(
          DATE '2022-06-03',
          date(date_trunc('week', current_date)) + 4,
          '1 week'
        ) as date
    )
    SELECT
      *
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    s"SELECT * FROM {default_db.x}"
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        x
    )
    SELECT
      *
    FROM
      table_0
    "###
    );
}

#[test]
fn test_direct_table_references() {
    assert_display_snapshot!(compile(
        r#"
    from x
    select s"{x}.field"
    "#,
    )
    .unwrap_err(), @r###"
    Error:
       ╭─[:3:14]
       │
     3 │     select s"{x}.field"
       │              ─┬─
       │               ╰─── table instance cannot be referenced directly
       │
       │ Help: did you forget to specify the column name?
    ───╯
    "###);

    assert_display_snapshot!(compile(
        r###"
    from x
    select x
    "###,
    )
    .unwrap(), @r###"
    SELECT
      *
    FROM
      x
    "###);
}

#[test]
fn test_name_shadowing() {
    assert_display_snapshot!(compile(
        r###"
    from x
    select {a, a, a = a + 1}
    "###).unwrap(),
        @r###"
    SELECT
      a AS _expr_0,
      a AS _expr_0,
      a + 1 AS a
    FROM
      x
    "###
    );

    assert_display_snapshot!(compile(
        r###"
    from x
    select a
    derive a
    derive a = a + 1
    derive a = a + 2
    "###).unwrap(),
        @r###"
    SELECT
      a AS _expr_0,
      a AS _expr_0,
      a + 1,
      a + 1 + 2 AS a
    FROM
      x
    "###
    );
}

#[test]
fn test_group_all() {
    assert_display_snapshot!(compile(
        r###"
    prql target:sql.sqlite

    from a=albums
    group a.* (aggregate {count this})
        "###).unwrap_err(), @r###"
    Error: Target dialect does not support * in this position.
    "###);

    assert_display_snapshot!(compile(
        r###"
    from e=albums
    group !{genre_id} (aggregate {count this})
        "###).unwrap_err(), @r###"
    Error: Excluding columns not supported as this position
    "###);
}

#[test]
fn test_output_column_deduplication() {
    // #1249
    assert_display_snapshot!(compile(
        r#"
    from report
    derive r = s"RANK() OVER ()"
    filter r == 1
        "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        *,
        RANK() OVER () AS r
      FROM
        report
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      r = 1
    "###
    );
}

#[test]
fn test_case() {
    assert_display_snapshot!(compile(
        r###"
    from employees
    derive display_name = case [
        nickname != null => nickname,
        true => f'{first_name} {last_name}'
    ]
        "###).unwrap(),
        @r###"
    SELECT
      *,
      CASE
        WHEN nickname IS NOT NULL THEN nickname
        ELSE CONCAT(first_name, ' ', last_name)
      END AS display_name
    FROM
      employees
    "###
    );

    assert_display_snapshot!(compile(
        r###"
    from employees
    derive display_name = case [
        nickname != null => nickname,
        first_name != null => f'{first_name} {last_name}'
    ]
        "###).unwrap(),
        @r###"
    SELECT
      *,
      CASE
        WHEN nickname IS NOT NULL THEN nickname
        WHEN first_name IS NOT NULL THEN CONCAT(first_name, ' ', last_name)
        ELSE NULL
      END AS display_name
    FROM
      employees
    "###
    );

    assert_display_snapshot!(compile(
        r###"
    from tracks
    select category = case [
        length > avg_length => 'long'
    ]
    group category (aggregate {count this})
        "###).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        CASE
          WHEN length > avg_length THEN 'long'
          ELSE NULL
        END AS category,
        length,
        avg_length
      FROM
        tracks
    )
    SELECT
      category,
      COUNT(*)
    FROM
      table_0
    GROUP BY
      category
    "###
    );
}

#[test]
fn test_sql_options() {
    let options = Options::default();
    let sql = prql_compiler::compile("from x", &options).unwrap();

    assert!(sql.contains('\n'));
    assert!(sql.contains("-- Generated by"));

    let options = Options::default().no_signature().no_format();
    let sql = prql_compiler::compile("from x", &options).unwrap();

    assert!(!sql.contains('\n'));
    assert!(!sql.contains("-- Generated by"));
}

#[test]
fn test_static_analysis() {
    assert_display_snapshot!(compile(
        r###"
    from x
    select {
        a = (- (-3)),
        b = !(!(!(!(!(true))))),
        a3 = null ?? y,

        a3 = case [
            false == true => 1,
            7 == 3 => 2,
            7 == y => 3,
            7.3 == 7.3 => 4,
            z => 5,
            true => 6
        ],
    }
        "###).unwrap(),
        @r###"
    SELECT
      3 AS a,
      false AS b,
      y AS _expr_0,
      CASE
        WHEN 7 = y THEN 3
        ELSE 4
      END AS a3
    FROM
      x
    "###
    );
}

#[test]
fn test_closures_and_pipelines() {
    assert_display_snapshot!(compile(
        r#"
    let addthree = a b c -> s"{a} || {b} || {c}"
    let arg = myarg myfunc <func> -> ( myfunc myarg )

    from y
    select x = (
        addthree "apples"
        arg "bananas"
        arg "citrus"
    )
        "#).unwrap(),
        @r###"
    SELECT
      'apples' || 'bananas' || 'citrus' AS x
    FROM
      y
    "###
    );
}

#[test]
fn test_basic_agg() {
    assert_display_snapshot!(compile(r#"
    from employees
    aggregate {
      count salary,
      count this,
    }
    "#).unwrap(),
        @r###"
    SELECT
      COUNT(*),
      COUNT(*)
    FROM
      employees
    "###
    );
}

#[test]
fn test_exclude_columns_01() {
    assert_display_snapshot!(compile(r#"
    from tracks
    select {track_id, title, composer, bytes}
    select !{title, composer}
    "#).unwrap(),
        @r###"
    SELECT
      track_id,
      bytes
    FROM
      tracks
    "###
    );
}

#[test]
fn test_exclude_columns_02() {
    assert_display_snapshot!(compile(r#"
    from tracks
    select {track_id, title, composer, bytes}
    group !{title, composer} (aggregate {count this})
    "#).unwrap(),
        @r###"
    SELECT
      track_id,
      bytes,
      COUNT(*)
    FROM
      tracks
    GROUP BY
      track_id,
      bytes
    "###
    );
}

#[test]
fn test_exclude_columns_03() {
    assert_display_snapshot!(compile(r#"
    from artists
    derive nick = name
    select !{artists.*}
    "#).unwrap(),
        @r###"
    SELECT
      name AS nick
    FROM
      artists
    "###
    );
}

#[test]
fn test_exclude_columns_04() {
    assert_display_snapshot!(compile(r#"
    prql target:sql.bigquery
    from tracks
    select !{milliseconds,bytes}
    "#).unwrap(),
        @r###"
    SELECT
      *
    EXCEPT
      (milliseconds, bytes)
    FROM
      tracks
    "###
    );
}

#[test]
fn test_exclude_columns_05() {
    assert_display_snapshot!(compile(r#"
    prql target:sql.snowflake
    from tracks
    select !{milliseconds,bytes}
    "#).unwrap(),
        @r###"
    SELECT
      * EXCLUDE (milliseconds, bytes)
    FROM
      tracks
    "###
    );
}

#[test]
fn test_exclude_columns_06() {
    assert_display_snapshot!(compile(r#"
    prql target:sql.duckdb
    from tracks
    select !{milliseconds,bytes}
    "#).unwrap(),
        @r###"
    SELECT
      * EXCLUDE (milliseconds, bytes)
    FROM
      tracks
    "###
    );
}

#[test]
fn test_exclude_columns_07() {
    assert_display_snapshot!(compile(r#"
    prql target:sql.duckdb
    from s"SELECT * FROM foo"
    select !{bar}
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        foo
    )
    SELECT
      * EXCLUDE (bar)
    FROM
      table_0
    "###
    );
}

#[test]
fn test_custom_transforms() {
    assert_display_snapshot!(compile(r#"
    let my_transform = (
        derive double = single * 2
        sort name
    )

    from tab
    my_transform
    take 3
    "#).unwrap(),
        @r###"
    SELECT
      *,
      single * 2 AS double
    FROM
      tab
    ORDER BY
      name
    LIMIT
      3
    "###
    );
}

#[test]
fn test_name_inference() {
    assert_display_snapshot!(compile(r#"
    from albums
    select {artist_id + album_id}
    # nothing inferred infer
    "#).unwrap(),
        @r###"
    SELECT
      artist_id + album_id
    FROM
      albums
    "###
    );

    let sql1 = compile(
        r#"
    from albums
    select {artist_id}
    # infer albums.artist_id
    select {albums.artist_id}
    "#,
    )
    .unwrap();
    let sql2 = compile(
        r#"
    from albums
    select {albums.artist_id}
    # infer albums.artist_id
    select {albums.artist_id}
    "#,
    )
    .unwrap();
    assert_eq!(sql1, sql2);

    assert_display_snapshot!(
        sql1,
        @r###"
    SELECT
      artist_id
    FROM
      albums
    "###
    );
}

#[test]
fn test_from_text() {
    assert_display_snapshot!(compile(r#"
    from_text format:csv """
a,b,c
1,2,3
4,5,6
    """
    select {b, c}
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        '1' AS a,
        '2' AS b,
        '3' AS c
      UNION
      ALL
      SELECT
        '4' AS a,
        '5' AS b,
        '6' AS c
    )
    SELECT
      b,
      c
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    from_text format:json '''
      [{"a": 1, "b": "x", "c": false }, {"a": 4, "b": "y", "c": null }]
    '''
    select {b, c}
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        1 AS a,
        'x' AS b,
        false AS c
      UNION
      ALL
      SELECT
        4 AS a,
        'y' AS b,
        NULL AS c
    )
    SELECT
      b,
      c
    FROM
      table_0
    "###
    );

    assert_display_snapshot!(compile(r#"
    from_text format:json '''{
        "columns": ["a", "b", "c"],
        "data": [
            [1, "x", false],
            [4, "y", null]
        ]
    }'''
    select {b, c}
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        1 AS a,
        'x' AS b,
        false AS c
      UNION
      ALL
      SELECT
        4 AS a,
        'y' AS b,
        NULL AS c
    )
    SELECT
      b,
      c
    FROM
      table_0
    "###
    );
}

#[test]
fn test_header() {
    // Test both target & version at the same time
    let header = format!(
        r#"
            prql target:sql.mssql version:"{}.{}"
            "#,
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    assert_display_snapshot!(compile(format!(r#"
    {header}

    from a
    take 5
    "#).as_str()).unwrap(),@r###"
    SELECT
      TOP (5) *
    FROM
      a
    "###);
}
#[test]
fn test_header_target_error() {
    assert_display_snapshot!(compile(r#"
    prql target:foo
    from a
    "#).unwrap_err(),@r###"
    Error: target `"foo"` not found
    "###);

    assert_display_snapshot!(compile(r#"
    prql target:sql.foo
    from a
    "#).unwrap_err(),@r###"
    Error: target `"sql.foo"` not found
    "###);

    assert_display_snapshot!(compile(r#"
    prql target:foo.bar
    from a
    "#).unwrap_err(),@r###"
    Error: target `"foo.bar"` not found
    "###);

    // TODO: Can we use the span of:
    // - Ideally just `dialect`?
    // - At least not the first empty line?
    assert_display_snapshot!(compile(r#"
    prql dialect:foo.bar
    from a
    "#).unwrap_err(),@r###"
    Error:
       ╭─[:1:1]
       │
     1 │ ╭─▶
     2 │ ├─▶     prql dialect:foo.bar
       │ │
       │ ╰────────────────────────────── unknown query definition arguments `dialect`
    ───╯
    "###);
}

#[test]
fn prql_version() {
    assert_display_snapshot!(compile(r#"
    from x
    derive y = std.prql_version
    "#).unwrap(),@r###"
    SELECT
      *,
      '0.11.0' AS y
    FROM
      x
    "###);
}

#[test]

fn shortest_prql_version() {
    assert_display_snapshot!(compile(r#"[{version = prql_version}]"#).unwrap(),@r###"
    WITH table_0 AS (
      SELECT
        '0.11.0' AS version
    )
    SELECT
      version
    FROM
      table_0
    "###);
}

#[test]
fn test_loop() {
    assert_display_snapshot!(compile(r#"
    from [{n = 1}]
    select n = n - 2
    loop (
        select n = n+1
        filter n<5
    )
    select n = n * 2
    take 4
    "#).unwrap(),
        @r###"
    WITH RECURSIVE table_0 AS (
      SELECT
        1 AS n
    ),
    table_1 AS (
      SELECT
        n - 2 AS _expr_0
      FROM
        table_0
      UNION
      ALL
      SELECT
        _expr_1
      FROM
        (
          SELECT
            _expr_0 + 1 AS _expr_1
          FROM
            table_1
        ) AS table_4
      WHERE
        _expr_1 < 5
    )
    SELECT
      _expr_0 * 2 AS n
    FROM
      table_1 AS table_3
    LIMIT
      4
    "###
    );
}

#[test]
fn test_loop_2() {
    assert_display_snapshot!(compile(r#"
    from (read_csv 'employees.csv')
    filter last_name=="Mitchell"
    loop (
      join manager=employees (manager.employee_id==this.reports_to)
      select manager.*
    )
    "#).unwrap(),
        @r###"
    WITH RECURSIVE table_0 AS (
      SELECT
        *
      FROM
        read_csv('employees.csv')
    ),
    table_1 AS (
      SELECT
        *
      FROM
        table_0
      WHERE
        last_name = 'Mitchell'
      UNION
      ALL
      SELECT
        manager.*
      FROM
        table_1
        JOIN employees AS manager ON manager.employee_id = table_1.reports_to
    )
    SELECT
      *
    FROM
      table_1 AS table_2
    "###
    );
}

#[test]
fn test_params() {
    assert_display_snapshot!(compile(r#"
    from i = invoices
    filter $1 <= i.date || i.date <= $2
    select {
        i.id,
        i.total,
    }
    filter i.total > $3
    "#).unwrap(),
        @r###"
    SELECT
      id,
      total
    FROM
      invoices AS i
    WHERE
      (
        $1 <= date
        OR date <= $2
      )
      AND total > $3
    "###
    )
}

// for #1969
#[test]
fn test_datetime() {
    let query = &r#"
        from test_table
        select {date = @2022-12-31, time = @08:30, timestamp = @2020-01-01T13:19:55-0800}
        "#;

    assert_snapshot!(
                compile(query).unwrap(),
                @r###"SELECT
  DATE '2022-12-31' AS date,
  TIME '08:30' AS time,
  TIMESTAMP '2020-01-01T13:19:55-0800' AS timestamp
FROM
  test_table
"###
    )
}

#[test]
fn test_datetime_sqlite() {
    // for #1969

    assert_snapshot!(compile(r#"
    prql target:sql.sqlite

    from x
    select {
        date = @2022-12-31,
        time = @08:30,
        time_tz = @03:05+08:00,
        time_tz2 = @03:05+0800,
        timestamp1 = @2020-01-01T13:19:55-0800,
        timestamp2 = @2021-03-14T03:05+0800,
        timestamp3 = @2021-03-14T03:05+08:00,
    }
    "#).unwrap(),
        @r###"
    SELECT
      DATE('2022-12-31') AS date,
      TIME('08:30') AS time,
      TIME('03:05+08:00') AS time_tz,
      TIME('03:05+08:00') AS time_tz2,
      DATETIME('2020-01-01T13:19:55-08:00') AS timestamp1,
      DATETIME('2021-03-14T03:05+08:00') AS timestamp2,
      DATETIME('2021-03-14T03:05+08:00') AS timestamp3
    FROM
      x
    "###
    );
}

#[test]
fn test_datetime_parsing() {
    assert_display_snapshot!(compile(r#"
    from test_tables
    select {date = @2022-12-31, time = @08:30, timestamp = @2020-01-01T13:19:55-0800}
    "#).unwrap(),
        @r###"
    SELECT
      DATE '2022-12-31' AS date,
      TIME '08:30' AS time,
      TIMESTAMP '2020-01-01T13:19:55-0800' AS timestamp
    FROM
      test_tables
    "###
    );
}

#[test]
fn test_lower() {
    assert_display_snapshot!(compile(r#"
    from test_tables
    derive {lower_name = (name | text.lower)}
    "#).unwrap(),
        @r###"
    SELECT
      *,
      LOWER(name) AS lower_name
    FROM
      test_tables
    "###
    );
}

#[test]
fn test_upper() {
    assert_display_snapshot!(compile(r#"
    from test_tables
    derive {upper_name = text.upper name}
    select {upper_name}
    "#).unwrap(),
        @r###"
    SELECT
      UPPER(name) AS upper_name
    FROM
      test_tables
    "###
    );
}

#[test]
fn test_1535() {
    assert_display_snapshot!(compile(r#"
    from x.y.z
    "#).unwrap(),
        @r###"
    SELECT
      *
    FROM
      x.y.z
    "###
    );
}

#[test]
fn test_read_parquet_duckdb() {
    assert_display_snapshot!(compile(r#"
    from (read_parquet 'x.parquet')
    join (read_parquet "y.parquet") (==foo)
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        read_parquet('x.parquet')
    ),
    table_1 AS (
      SELECT
        *
      FROM
        read_parquet('y.parquet')
    )
    SELECT
      table_0.*,
      table_1.*
    FROM
      table_0
      JOIN table_1 ON table_0.foo = table_1.foo
    "###
    );

    // TODO: `from x=(read_parquet 'x.parquet')` currently fails
}

#[test]
fn test_excess_columns() {
    // https://github.com/PRQL/prql/issues/2079
    assert_display_snapshot!(compile(r#"
    from tracks
    derive d = track_id
    sort d
    select {title}
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        title,
        track_id AS _expr_0
      FROM
        tracks
    )
    SELECT
      title
    FROM
      table_0
    ORDER BY
      _expr_0
    "###
    );
}

#[test]
fn test_regex_search() {
    assert_display_snapshot!(compile(r#"
    from tracks
    derive is_bob_marley = artist_name ~= "Bob\\sMarley"
    "#).unwrap(),
        @r###"
    SELECT
      *,
      REGEXP(artist_name, 'Bob\sMarley') AS is_bob_marley
    FROM
      tracks
    "###
    );
}

#[test]
fn test_intervals() {
    assert_display_snapshot!(compile(r#"
    from foo
    select dt = 1years + 1months + 1weeks + 1days + 1hours + 1minutes + 1seconds + 1milliseconds + 1microseconds
    "#).unwrap(),
        @r###"
    SELECT
      INTERVAL 1 YEAR + INTERVAL 1 MONTH + INTERVAL 1 WEEK + INTERVAL 1 DAY + INTERVAL 1 HOUR + INTERVAL 1 MINUTE + INTERVAL 1 SECOND + INTERVAL 1 MILLISECOND + INTERVAL 1 MICROSECOND AS dt
    FROM
      foo
    "###
    );
}

#[test]
fn test_into() {
    assert_display_snapshot!(compile(r#"
    from data
    into table_a

    from table_a
    select {x, y}
    "#).unwrap(),
        @r###"
    WITH table_a AS (
      SELECT
        *
      FROM
        data
    )
    SELECT
      x,
      y
    FROM
      table_a
    "###
    );
}

#[test]
fn test_array() {
    assert_display_snapshot!(compile(r#"
    let a = [1, 2, false]
    "#).unwrap_err(),
        @r###"
    Error:
       ╭─[:2:20]
       │
     2 │     let a = [1, 2, false]
       │                    ──┬──
       │                      ╰──── array expected type `int`, but found type `bool`
    ───╯
    "###
    );

    assert_snapshot!(compile(r#"
    let my_relation = [
        {a = 3, b = false},
        {a = 4, b = true},
    ]

    let main = (my_relation | filter b)
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        3 AS a,
        false AS b
      UNION
      ALL
      SELECT
        4 AS a,
        true AS b
    ),
    my_relation AS (
      SELECT
        a,
        b
      FROM
        table_0
    )
    SELECT
      a,
      b
    FROM
      my_relation
    WHERE
      b
    "###
    );
}

#[test]
fn test_double_stars() {
    assert_display_snapshot!(compile(r#"
    from tb1
    join tb2 (==c2)
    take 5
    filter (tb2.c3 < 100)
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        tb1.*,
        tb2.*
      FROM
        tb1
        JOIN tb2 ON tb1.c2 = tb2.c2
      LIMIT
        5
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      c3 < 100
    "###
    );

    assert_display_snapshot!(compile(r#"
    prql target:sql.duckdb

    from tb1
    join tb2 (==c2)
    take 5
    filter (tb2.c3 < 100)
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        tb1.*,
        tb2.*
      FROM
        tb1
        JOIN tb2 ON tb1.c2 = tb2.c2
      LIMIT
        5
    )
    SELECT
      *
    FROM
      table_0
    WHERE
      c3 < 100
    "###
    );
}

#[test]
fn test_lineage() {
    // #2627
    assert_display_snapshot!(compile(r#"
    from_text """
    a
    1
    2
    3
    """
    derive a = a
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        '    1' AS a
      UNION
      ALL
      SELECT
        '    2' AS a
      UNION
      ALL
      SELECT
        '    3' AS a
    )
    SELECT
      a,
      a
    FROM
      table_0
    "###
    );

    // #2392
    assert_display_snapshot!(compile(r#"
    from_text format:json """{
        "columns": ["a"],
        "data": [[1]]
    }"""
    derive a = a + 1
    "#).unwrap(),
        @r###"
    WITH table_0 AS (
      SELECT
        1 AS a
    )
    SELECT
      a AS _expr_0,
      a + 1 AS a
    FROM
      table_0
    "###
    );
}

#[test]
fn test_type_as_column_name() {
    // #2503
    assert_display_snapshot!(compile(r#"
    let f = tbl -> (
      t = tbl
      select t.date
    )

    from foo
    f"#)
    .unwrap(), @r###"
    SELECT
      date
    FROM
      foo AS t
    "###);
}

#[test]
fn test_error_code() {
    let err = compile(
        r###"
    let a = (from x)
    "###,
    )
    .unwrap_err();
    assert_eq!(err.inner[0].code.as_ref().unwrap(), "E0001");
}

#[test]
fn large_query() {
    // This was causing a stack overflow on Windows, ref https://github.com/PRQL/prql/issues/2857
    compile(
        r###"
from employees
filter gross_cost > 0
group {title} (
  aggregate {
    ct = count this,
  }
)
sort ct
filter ct > 200
take 20
sort ct
filter ct > 200
take 20
sort ct
filter ct > 200
take 20
sort ct
filter ct > 200
take 20
    "###,
    )
    .unwrap();
}

#[test]
fn test_returning_constants_only() {
    assert_display_snapshot!(compile(
        r###"
    from tb1
    sort {a}
    select {c = b}
    select {d = 10}
    "###,
    )
    .unwrap(), @r###"
    WITH table_0 AS (
      SELECT
        10 AS d,
        a
      FROM
        tb1
    )
    SELECT
      d
    FROM
      table_0
    ORDER BY
      a
    "###);

    assert_display_snapshot!(compile(
        r###"
    from tb1
    take 10
    filter true
    take 20
    filter true
    select d = 10
    "###,
    )
    .unwrap(), @r###"
    WITH table_1 AS (
      SELECT
        NULL
      FROM
        tb1
      LIMIT
        10
    ), table_0 AS (
      SELECT
        NULL
      FROM
        table_1
      WHERE
        true
      LIMIT
        20
    )
    SELECT
      10 AS d
    FROM
      table_0
    WHERE
      true
    "###);
}

#[test]
fn test_conflicting_names_at_split() {
    // issue #2697
    assert_display_snapshot!(compile(
        r#"
    from s = workflow_steps
    join wp=workflow_phases (s.phase_id == wp.id)
    filter wp.name == "CREATE_OUTLET"
    join w=workflow (wp.workflow_id == w.id)
    select {
        step_id = s.id,
        phase_id = wp.id,
    }
    "#,
    )
    .unwrap(), @r###"
    WITH table_0 AS (
      SELECT
        wp.id,
        s.id AS _expr_0,
        wp.workflow_id
      FROM
        workflow_steps AS s
        JOIN workflow_phases AS wp ON s.phase_id = wp.id
      WHERE
        wp.name = 'CREATE_OUTLET'
    )
    SELECT
      table_0._expr_0 AS step_id,
      table_0.id AS phase_id
    FROM
      table_0
      JOIN workflow AS w ON table_0.workflow_id = w.id
    "###);
}

#[test]
fn test_relation_literal_quoting() {
    // issue #3484
    assert_display_snapshot!(compile(
        r###"
    from [
        {`small number`=1e-10, `large number`=1e10},
    ]
    select {`small number`, `large number`}
    "###,
    )
    .unwrap(), @r###"
    WITH table_0 AS (
      SELECT
        1e-10 AS "small number",
        10000000000.0 AS "large number"
    )
    SELECT
      "small number",
      "large number"
    FROM
      table_0
    "###);
}

#[test]
fn test_relation_var_name_clashes_01() {
    assert_display_snapshot!(compile(
        r###"
    let table_0 = (from a)

    from table_0
    take 10
    filter x > 0
        "###,
    )
    .unwrap(), @r###"
    WITH table_0 AS (
      SELECT
        *
      FROM
        a
    ),
    table_1 AS (
      SELECT
        *
      FROM
        table_0
      LIMIT
        10
    )
    SELECT
      *
    FROM
      table_1
    WHERE
      x > 0
    "###);
}

#[test]
fn test_relation_var_name_clashes_02() {
    // issue #3713
    assert_display_snapshot!(compile(
        r###"
    from t
    join t (==x)
        "###,
    )
    .unwrap(), @r###"
    SELECT
      t.*,
      table_0.*
    FROM
      t
      JOIN t AS table_0 ON t.x = table_0.x
    "###);
}

#[test]
#[ignore]
fn test_select_this() {
    // Currently broken for a few reasons:
    // - type of `this` is not resolved as tuple, but an union?
    // - lineage is not computed correctly
    assert_display_snapshot!(compile(
        r###"
    from x
    select {a, b}
    select this
        "###,
    )
    .unwrap(), @r###"
    SELECT
      a,
      b
    FROM
      x
    "###);
}

#[test]
fn test_group_exclude() {
    assert_display_snapshot!(compile(
        r###"
    from x
    select {a, b}
    group {a} (derive c = a + 1)
        "###,
    )
    .unwrap_err(), @r###"
    Error:
       ╭─[:4:27]
       │
     4 │     group {a} (derive c = a + 1)
       │                           ┬
       │                           ╰── Unknown name `a`
    ───╯
    "###);

    // assert_display_snapshot!(compile(
    //     r###"
    // from x
    // select {a, b}
    // group {a + 1} (aggregate {sum b})
    //     "###,
    // )
    // .unwrap_err(), @r###"
    // SELECT
    //   a,
    //   b
    // FROM
    //   x
    // "###);
}
