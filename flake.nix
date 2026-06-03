{
  description = "Cross-platform Rust service reporting PC activity and connected monitors to MQTT for Home Assistant";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs = { self, nixpkgs, systems }: let
    eachSystem = f: nixpkgs.lib.genAttrs (import systems) (system: f nixpkgs.legacyPackages.${system});
  in {
    packages = eachSystem (pkgs: rec {
      default = hass-pc-mon;
      hass-pc-mon = pkgs.rustPlatform.buildRustPackage {
        pname = "hass-pc-mon";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;

        nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.pkg-config ];

        # macOS CoreFoundation / CoreGraphics / IOKit come from the default
        # apple-sdk pulled in by stdenv; no explicit framework deps needed.
        buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
          pkgs.xorg.libX11
          pkgs.xorg.libXScrnSaver
        ];

        # Tests need real HID / IOKit / a display — skip in the sandbox.
        doCheck = false;

        meta = with pkgs.lib; {
          description = "Cross-platform PC activity + monitor reporter for Home Assistant over MQTT";
          homepage = "https://github.com/winterscar/hass-pc-mon";
          license = licenses.mit;
          platforms = platforms.darwin ++ platforms.linux;
          mainProgram = "hass-pc-mon";
        };
      };
    });

    devShells = eachSystem (pkgs: {
      default = pkgs.mkShell {
        inputsFrom = [ self.packages.${pkgs.system}.default ];
        packages = [
          pkgs.rustc
          pkgs.cargo
          pkgs.clippy
          pkgs.rustfmt
          pkgs.rust-analyzer
        ];
      };
    });

    darwinModules.default = import ./nix/darwin-module.nix self;
    nixosModules.default = import ./nix/nixos-module.nix self;

    formatter = eachSystem (pkgs: pkgs.nixfmt);
  };
}
