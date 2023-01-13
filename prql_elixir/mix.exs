defmodule PRQL.MixProject do
  use Mix.Project

  def project do
    [
      app: :prql,
      version: "0.1.0",
      elixir: "~> 1.14",
      deps: deps()
    ]
  end

  # Run "mix help compile.app" to learn about applications.
  def application do
    [
      extra_applications: [:logger]
    ]
  end

  # Run "mix help deps" to learn about dependencies.
  defp deps do
    [
      {:rustler, "~> 0.26.0"}
    ]
  end
end
