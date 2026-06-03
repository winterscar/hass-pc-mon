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

    logPath = lib.mkOption {
      type = lib.types.str;
      default = "/tmp/hass-pc-mon.log";
      description = "stdout/stderr destination for the launchd agent.";
    };
  };

  config = lib.mkIf cfg.enable {
    launchd.user.agents.hass-pc-mon = {
      serviceConfig = {
        Label = "com.hass-pc-mon";
        ProgramArguments = [
          (lib.getExe cfg.package)
          "--config"
          "${configFile}"
          "run"
        ];
        RunAtLoad = true;
        KeepAlive = true;
        StandardOutPath = cfg.logPath;
        StandardErrorPath = cfg.logPath;
      };
    };

    environment.systemPackages = [ cfg.package ];
  };
}
