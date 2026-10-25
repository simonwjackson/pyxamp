{
  description = "Pyxamp universal MPRIS remote";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    webamp = { url = "github:captbaritone/webamp/7734cd3c7cf144bab2d8614ce9f9ba2ff21e163e"; flake = false; };
    wacup = { url = "github:WACUP/vis_classic"; flake = false; };
  };
  outputs = { self, nixpkgs, ... }: let
    systems = [ "x86_64-linux" "aarch64-linux" ];
    each = nixpkgs.lib.genAttrs systems;
  in {
    homeManagerModules.default = import ./nix/home-manager.nix self;
    packages = each (system: let
      pkgs = import nixpkgs { inherit system; };
      daemon = pkgs.rustPlatform.buildRustPackage {
        pname = "pyxamp-daemon";
        version = "0.1.0";
        src = self;
        cargoLock.lockFile = ./Cargo.lock;
        nativeBuildInputs = [ pkgs.pkg-config pkgs.rustPlatform.bindgenHook pkgs.makeWrapper ];
        buildInputs = [ pkgs.pipewire ];
        doCheck = false; # Focused/combined verification runs explicitly under an isolated bus.
        meta = {
          description = "Universal MPRIS remote with targeted PipeWire analysis";
          license = [ pkgs.lib.licenses.mit pkgs.lib.licenses.bsd3 ];
          platforms = systems;
          mainProgram = "pyxamp-daemon";
        };
        postInstall = ''
          mkdir -p $out/share/doc/pyxamp/licenses
          cp daemon/licenses/* $out/share/doc/pyxamp/licenses/
          cp contracts/pyxamp.schema.json $out/share/doc/pyxamp/
          # Own playback and the Jellyfin video hand-off use this plain mpv (no scripts).
          # The hand-off adds the MPRIS script itself, so the video is a followed player.
          # The volume slider sets the system volume with WirePlumber's wpctl.
          wrapProgram $out/bin/pyxamp-daemon \
            --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.mpv pkgs.wireplumber ]} \
            --set-default PYXAMP_MPV ${pkgs.mpv}/bin/mpv \
            --set-default PYXAMP_MPV_MPRIS ${pkgs.mpvScripts.mpris}/share/mpv/scripts/mpris.so
        '';
      };
    in {
      inherit daemon;
      default = pkgs.symlinkJoin {
        name = "pyxamp";
        paths = [ daemon ];
        passthru.quickshell = pkgs.quickshell;
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          test -f ${self}/face/shell.qml || { echo 'Combined package requires merged face/shell.qml' >&2; exit 1; }
          mkdir -p $out/etc/xdg/quickshell/pyxamp
          cp -r ${self}/face/. $out/etc/xdg/quickshell/pyxamp/
          makeWrapper ${pkgs.quickshell}/bin/qs $out/bin/pyxamp \
            --set-default XDG_CONFIG_DIRS /etc/xdg \
            --prefix XDG_CONFIG_DIRS : "$out/etc/xdg" \
            --add-flags '-n -c pyxamp'
        '';
      };
    });
    devShells = each (system: let pkgs = import nixpkgs { inherit system; }; in {
      browser = pkgs.mkShell {
        inputsFrom = [ self.devShells.${system}.default ];
        nativeBuildInputs = [ pkgs.nodejs_24 pkgs.pulseaudio pkgs.xorg-server pkgs.curl ];
      };
      default = pkgs.mkShell {
        nativeBuildInputs = [ pkgs.cargo pkgs.rustc pkgs.rustfmt pkgs.clippy pkgs.pkg-config pkgs.rustPlatform.bindgenHook pkgs.dbus pkgs.pipewire pkgs.wireplumber pkgs.util-linux pkgs.jq pkgs.mpv ];
        buildInputs = [ pkgs.pipewire ];
        CARGO_BUILD_JOBS = "2";
        PYXAMP_MPV_MPRIS = "${pkgs.mpvScripts.mpris}/share/mpv/scripts/mpris.so";
      };
    });
  };
}
