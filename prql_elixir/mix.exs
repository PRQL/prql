defmodule PRQL.MixProject do
  use Mix.Project

  def project do
    [
      app: :prql,
      version: "0.1.0",
      elixir: "~> 1.14",
      deps: deps(),
      name: "PRQL",
      source_url: "https://github.com/PRQL/prql",
      homepage_url: "https://prql-lang.org/",
      docs: [
        # The main page in the docs
        main: "readme",
        logo: "../website/static/img/icon.svg",
        extras: ["README.md"]
      ]
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
      {:rustler, "~> 0.26.0"},
      {:ex_doc, "~> 0.21", only: :dev, runtime: false}
    ]
  end
end
