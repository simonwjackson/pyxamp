self: { config, lib, pkgs, ... }:
let cfg = config.programs.pyxamp;
in {
  options.programs.pyxamp = {
    enable = lib.mkEnableOption "Pyxamp's universal remote daemon and Quickshell face";
    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Combined Pyxamp package.";
    };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
    xdg.configFile."quickshell/pyxamp".source = "${cfg.package}/etc/xdg/quickshell/pyxamp";
    systemd.user.services.pyxamp-daemon = {
      Unit = {
        Description = "Pyxamp universal MPRIS remote";
        After = [ "graphical-session.target" "pipewire.service" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/pyxamp-daemon";
        Restart = "on-failure";
        RestartSec = 2;
        UMask = "0077";
        NoNewPrivileges = true;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
