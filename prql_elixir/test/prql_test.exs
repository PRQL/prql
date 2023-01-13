defmodule PRQLTest do
  use ExUnit.Case
  doctest PRQL

  test "greets the world" do
    assert PRQL.hello() == :world
  end
end
