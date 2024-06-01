defmodule PRQLTest do
  use ExUnit.Case
  doctest PRQL

  @compile_opts [signature_comment: false]

  test "compiles PRQL" do
    prql_query = "from db.customers"

    assert PRQL.compile(prql_query, @compile_opts) ==
             {:ok,
              """
              SELECT
                *
              FROM
                customers
              """}
  end

  test "return errors on invalid query" do
    {:ok, expected_json} =
      Jason.decode(~S"""
      {
        "inner": [
          {
            "kind": "Error",
            "code": null,
            "reason": "Unknown name `invalid`",
            "hints": [],
            "span": "1:0-7",
            "display": "Error: \n   ╭─[:1:1]\n   │\n 1 │ invalid\n   │ ───┬───  \n   │    ╰───── Unknown name `invalid`\n───╯\n",
            "location": {
              "start": [0, 0],
              "end": [0, 7]
            }
          }
        ]
      }
      """)

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
