defmodule PRQLTest do
  use ExUnit.Case
  doctest PRQL

  @compile_opts [signature_comment: false]

  test "compiles PRQL" do
    prql_query = """
      from customers
    """

    excepted_result = "SELECT\n  *\nFROM\n  customers\n"

    assert PRQL.compile(prql_query, @compile_opts) == {:ok, excepted_result}
  end

  test "return errors on invalid query" do
    expected_result = ~S"""
    {
      "inner": [
        {
          "kind": "Error",
          "code": null,
          "reason": "Unknown name invalid",
          "hint": null,
          "span": {
            "start": 0,
            "end": 7
          },
          "display": "Error: \n   ╭─[:1:1]\n   │\n 1 │ invalid\n   │ ───┬───  \n   │    ╰───── Unknown name invalid\n───╯\n",
          "location": {
            "start": [0, 0],
            "end": [0, 7]
          }
        }
      ]
    }
    """

    {:ok, expected_json} = Jason.decode(expected_result)
    result = PRQL.compile("invalid", @compile_opts)

    case result do
      {:error, error_string} ->
        {:ok, error_json} = Jason.decode(error_string)
        assert error_json == expected_json

      _ ->
        flunk("Expected an error tuple")
    end
  end
end
