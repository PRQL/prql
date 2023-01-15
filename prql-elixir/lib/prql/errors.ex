defmodule PRQL.PRQLError do
  @moduledoc """
  Represents an error returned from PRQL compiler.

  `:error` contains the message from compiler as a **JSON string**.
  """
  defexception [:message, :error]

  @impl true
  def exception(err) do
    %__MODULE__{message: "Error compiling PRQL query", error: err}
  end
end
