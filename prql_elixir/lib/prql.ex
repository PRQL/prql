defmodule PRQL do
  @moduledoc """
  Documentation for `PRQL`.

  This module provide Elixir bindings for [PRQL](https://prql-lang.org/).

  PRQL is a modern language for transforming data.
  """
  alias PRQL.Native.CompileOptions

  @type dialect() ::
          :generic
          | :mssql
          | :mysql
          | :postgres
          | :ansi
          | :big_query
          | :click_house
          | :hive
          | :sql_lite
          | :snow_flake
  @type format_opt :: {:format, boolean()}
  @type signature_comment_opt :: {:signature_comment, boolean()}
  @type dialect_opt :: {:dialect, dialect()}
  @type compile_opts :: format_opt() | signature_comment_opt() | dialect_opt()

  @doc ~S"""
  Compile a `PRQL` query to `SQL` query.

  Returns generated SQL query on success in shape of `{:ok, sql}`. This SQL query can
  be safely feeded into a SQL driver to get the output.

  On error, `{:error, reason}` is returned where `reason` is a `JSON` string.
  Use any `JSON` encoder library to encode it.

  ## Options

    * `:dialect` - Dialect used for generate SQL. Accepted values are
    `:generic`, `:mssql`, `:mysql`, `:postgres`, `:ansi`, `:big_query`,
    `:click_house`, `:hive`, `:sql_lite`, `:snow_flake`

    * `:format` - Formats the output, defaults to `true`

    * `signature_comment` - Set the signature comment generated by PRQL, defaults to `true`


  ## Examples

  Using default `Generic` dialect:
      iex> PRQL.compile("from customers")
      {:ok, "SELECT\n  *\nFROM\n  customers\n\n-- Generated by PRQL compiler version 0.3.1 (https://prql-lang.org)\n"}


  Using `MSSQL` dialect:
      iex> PRQL.compile("from customers\ntake 10", dialect: :mssql)
      {:ok, "SELECT\n  TOP (10) *\nFROM\n  customers\n\n-- Generated by PRQL compiler version 0.3.1 (https://prql-lang.org)\n"}
  """
  @spec compile(binary(), [compile_opts()]) :: {:ok, binary()} | {:error, binary()}
  def compile(prql_query, opts \\ []) when is_binary(prql_query) and is_list(opts) do
    PRQL.Native.compile(prql_query, struct(CompileOptions, opts))
  end

  @doc """
  PRQL to PL AST
  """
  def prql_to_pl(prql_query) when is_binary(prql_query) do
    PRQL.Native.prql_to_pl(prql_query)
  end

  @doc """
  PL AST to RQ
  """
  def pl_to_rq(pl_json) do
    PRQL.Native.pl_to_rq(pl_json)
  end

  @doc """
  RQ to SQL
  """
  def rq_to_sql(rq_json) do
    PRQL.Native.rq_to_sql(rq_json)
  end
end
