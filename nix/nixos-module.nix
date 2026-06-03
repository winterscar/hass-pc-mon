self:
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hass-pc-mon;
  tomlFormat = pkgs.formats.toml { };
  configFile = tomlFormat.generate "hass-pc-mon.toml" cfg.settings;
in
{
  options.services.hass-pc-mon = {
    enable = lib.mkEnableOption "hass-pc-mon — report PC state to MQTT";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = lib.literalExpression "hass-pc-mon.packages.\${system}.default";
      description = "The hass-pc-mon package to run.";
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      description = ''
        Contents of the hass-pc-mon config file. Rendered to TOML and passed
        via `--config`. See README for the full schema.

        The service runs as a per-user systemd unit because idle detection
        requires access to the user's X11 session.
      '';
      example = lib.literalExpression ''
        {
          update_interval_secs = 30;
          idle_threshold_secs  = 120;
          mqtt = {
            host     = "10.0.0.5";
            username = "homeassistant";
            password = "hunter2";
          };
        }
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.hass-pc-mon = {
      description = "hass-pc-mon — report PC state to MQTT";
      wantedBy = [ "default.target" ];
      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --config ${configFile} run";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    environment.systemPackages = [ cfg.package ];
  };
}
