defmodule PRQLTest do
  use ExUnit.Case
  doctest PRQL

  @compile_opts [signature_comment: false]

  test "compiles PRQL" do
    prql_query = """
      from customers
    """

    excepted_result = "SELECT\n  *\nFROM\n  customers"

    assert PRQL.compile(prql_query, @compile_opts) == {:ok, excepted_result}
  end

  test "return errors on invalid query" do
    excepted_result =
      "{\"inner\":[{\"reason\":\"Unknown name invalid\",\"hint\":null,\"span\":{\"start\":0,\"end\":7},\"display\":\"Error: \\n   ╭─[:1:1]\\n   │\\n 1 │ invalid\\n   · ───┬───  \\n   ·    ╰───── Unknown name invalid\\n───╯\\n\",\"location\":{\"start\":[0,0],\"end\":[0,7]}}]}"

    assert PRQL.compile("invalid", @compile_opts) == {:error, excepted_result}
  end
end
