defmodule PRQL.Native do
  @moduledoc false
  use Rustler, otp_app: :prql

  def compile(_prql_query, _options), do: e()

  def prql_to_pl(_prql_query), do: e()

  def pl_to_rq(_pl_json), do: e()

  def rq_to_sql(_rq_json), do: e()

  defp e(), do: :erlang.nif_error(:nif_not_loaded)
end

defmodule PRQL.Native.CompileOptions do
  @typedoc """
  Dialect used for SQL generation
  """
  @type target() ::
          :generic
          | :mssql
          | :mysql
          | :postgres
          | :ansi
          | :bigquery
          | :clickhouse
          | :glaredb
          | :sqlite
          | :snowflake

  @type t :: %__MODULE__{
          target: target(),
          format: boolean(),
          signature_comment: boolean()
        }

  defstruct target: :generic, format: true, signature_comment: true
end
