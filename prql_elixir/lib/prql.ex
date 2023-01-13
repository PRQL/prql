defmodule PRQL do
  @moduledoc """
  Documentation for `PRQL`.
  """

  @spec compile(binary(), keyword()) :: {:ok, binary()} | {:error, binary()}
  def compile(prql_query, options \\ []) when is_binary(prql_query) and is_list(options) do
    PRQL.Native.compile(prql_query, options)
  end
end
