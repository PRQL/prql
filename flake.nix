{
  description = "PRQL development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    mdbook-footnote.url = "github:aljazerzen/mdbook-footnote";
    hyperlink.url = "github:aljazerzen/hyperlink";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, mdbook-footnote, hyperlink, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fenix_pkgs = fenix.packages.${system};

        essentials = with pkgs; [
          # requirements
          fenix_pkgs.stable.cargo
          fenix_pkgs.stable.clippy
          fenix_pkgs.stable.rust-src
          fenix_pkgs.stable.rustc
          fenix_pkgs.stable.rustfmt
          fenix_pkgs.stable.rust-analyzer

          # tooling
          clang # for llvm debugger in VSCode

          # tools
          cargo-nextest
          bacon
          cargo-audit
          cargo-insta
          cargo-release
          pkg-config
          openssl
          cargo-llvm-cov

          # actions
          go-task
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
          # bindings
          python311
          zlib
          maturin
          ruff
          black

          wasm-bindgen-cli
          wasm-pack
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

          # needed for running wheels produced by Python maturin builds that are not manylinux
          # shellHook = ''
          #   export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath bindings}:$LD_LIBRARY_PATH"
          #   export LD_LIBRARY_PATH="${pkgs.stdenv.cc.cc.lib.outPath}/lib:$LD_LIBRARY_PATH"
          # '';
        };
      });
}
