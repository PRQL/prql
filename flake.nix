{
  description = "PRQL development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    mdbook-footnote.url = "github:aljazerzen/mdbook-footnote";
    hyperlink.url = "github:aljazerzen/hyperlink";
  };

  outputs = { self, nixpkgs, flake-utils, mdbook-footnote, hyperlink }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        essentials = with pkgs; [
          # compiler requirements
          rustup
          clang

          # tools
          cargo-nextest
          bacon
          cargo-audit
          cargo-insta
          cargo-release

          # actions
          just
          sd
          ripgrep
          nodePackages.prettier
          #nodePackages.prettier-plugin-go-template
          #nixpkgs-fmt
          rsync
        ];

        web = with pkgs; [
          # book
          mdbook
          mdbook-admonish
          mdbook-footnote.defaultPackage.${system}

          # website
          hugo

          # playground
          nodejs
          nodePackages.npm

          # link check
          hyperlink.defaultPackage.${system}
        ];

        bindings = with pkgs; [
          # compiler bindings
          python311
          wasm-bindgen-cli
          wasm-pack
          maturin
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = essentials;
        };
        devShells.web = pkgs.mkShell {
          buildInputs = essentials ++ web;
        };
        devShells.full = pkgs.mkShell {
          buildInputs = essentials ++ web ++ bindings;
        };
      });
}
