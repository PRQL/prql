defmodule PRQL.Native do
  @moduledoc false
  use Rustler, otp_app: :prql

  def compile(_prql_query, _options), do: :erlang.nif_error(:nif_not_loaded)

  def prql_to_pl(_prql_query), do: :erlang.nif_error(:nif_not_loaded)

  def pl_to_rq(_pl_json), do: :erlang.nif_error(:nif_not_loaded)

  def rq_to_sql(_rq_json), do: :erlang.nif_error(:nif_not_loaded)
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
          | :big_query
          | :click_house
          | :hive
          | :sql_lite
          | :snow_flake

  @type t :: %__MODULE__{
          dialect: dialect(),
          format: boolean(),
          signature_comment: boolean()
        }

  defstruct dialect: :generic, format: true, signature_comment: true
end
