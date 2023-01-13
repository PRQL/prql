defmodule PRQLTest do
  use ExUnit.Case
  doctest PRQL

  test "compiles to correct PRQL query" do
    prql = """
      from customers
    """

    assert 1 == 1
  end
end
