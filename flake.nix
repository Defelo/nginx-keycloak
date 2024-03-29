{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-23.05";
  };
  outputs = {
    self,
    nixpkgs,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
  in {
    packages.${system} = let
      inherit (fromTOML (builtins.readFile ./Cargo.toml)) package;
    in {
      default = self.packages.${system}.nginx-keycloak;
      nginx-keycloak = pkgs.rustPlatform.buildRustPackage {
        pname = package.name;
        version = package.version;
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        doCheck = false;
      };
    };
    nixosModules = {
      default = self.nixosModules.nginx-keycloak;
      nginx-keycloak = {
        config,
        lib,
        ...
      }:
        with lib; let
          cfg = config.services.nginx-keycloak;
        in {
          options.services.nginx-keycloak = {
            enable = mkEnableOption "nginx-keycloak";
            redis = mkOption {
              type = types.bool;
              default = true;
            };
            debug = mkOption {
              type = types.bool;
              default = false;
            };
            settings = {
              host = mkOption {type = types.str;};
              port = mkOption {type = types.port;};
              keycloak_base_url = mkOption {type = types.str;};
              client_id = mkOption {type = types.str;};
              client_secret_file = mkOption {type = types.path;};
              auth_callback_path = mkOption {
                type = types.str;
                default = "/_auth/callback";
              };
              redis_url = mkOption {type = types.str;};
              session_allowed_ttl = mkOption {
                type = types.int;
                default = 60;
              };
              session_forbidden_ttl = mkOption {
                type = types.int;
                default = 10;
              };
            };
          };
          config = mkIf cfg.enable {
            services.redis = mkIf cfg.redis {
              servers.nginx-keycloak = {
                enable = true;
                save = [];
              };
            };
            services.nginx-keycloak.settings.redis_url = mkIf cfg.redis "redis+unix:///${config.services.redis.servers.nginx-keycloak.unixSocket}";
            systemd.services = {
              nginx-keycloak = {
                wantedBy = ["multi-user.target"];
                serviceConfig = {
                  User = "nginx-keycloak";
                  Group = "nginx-keycloak";
                  DynamicUser = true;
                  SupplementaryGroups = mkIf cfg.redis "redis-nginx-keycloak";
                  LoadCredential = ["client_secret:${cfg.settings.client_secret_file}"];
                };
                environment = {
                  CONFIG_PATH = pkgs.writeText "config.json" (builtins.toJSON (cfg.settings));
                  RUST_LOG =
                    if cfg.debug
                    then "debug"
                    else "info";
                };
                script = ''
                  CLIENT_SECRET_FILE=$CREDENTIALS_DIRECTORY/client_secret ${self.packages.${system}.nginx-keycloak}/bin/nginx-keycloak
                '';
              };
            };
          };
        };
    };
    lib = {
      auth_config = location: ''
        auth_request ${location};
        auth_request_set $auth_redirect $upstream_http_x_auth_redirect;
        auth_request_set $auth_cookie $upstream_http_x_auth_cookie;
        error_page 401 =307 $auth_redirect;
        more_set_headers "Set-Cookie: $auth_cookie";
      '';
      auth_location = {
        host,
        role,
      }: {
        extraConfig = ''
          internal;
          proxy_pass ${host}/auth?role=${role};
          proxy_pass_request_body off;
          proxy_set_header Content-Length "";
          proxy_set_header X-Request-Uri $scheme://$host$request_uri;
        '';
      };
    };
  };
}
