defmodule PRQL.Native do
  use Rustler, otp_app: :prql, crate: "prql"

  # When your NIF is loaded, it will override this function.
  def compile(_prql_query, _options \\ []), do: :erlang.nif_error(:nif_not_loaded)
end

defmodule PRQL.Native.CompileOptions do
  defstruct [:dialect, :format, :signature_comment]
end
