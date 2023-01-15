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
  @type dialect() ::
          :generic
          | :mssql
          | :mysql
          | :postgres
          | :ansi
          | :bigquery
          | :clickhouse
          | :hive
          | :sqllite
          | :snowflake

  @type t :: %__MODULE__{
          dialect: dialect(),
          format: boolean(),
          signature_comment: boolean()
        }

  defstruct dialect: :generic, format: true, signature_comment: true
end
