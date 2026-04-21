# ArmaField-Windows-Server

Native Windows dedicated server launcher for Arma Reforger with native support for the [ArmaField](https://reforger.armaplatform.com/workshop/68FA258A6C74CE73-ArmaField) mod's MapSeeding rotation mechanic.

- Auto-installs and updates the game via SteamCMD
- Reads `config.json` and patches bind addresses, ports, and scenario ID
- Rotates missions between rounds via `MapSeeding.json` written by the mod
- Runs as a console app or as a Windows Service
- Idempotent Windows Firewall rule management

## Requirements

- Windows 10 / 11 / Server 2019+ (x64)
- At least 4 GB free RAM (6 GB recommended)
- At least 15 GB free disk (game files ~10 GB plus mods)
- 1 Gbps (1000 Mbit/s) upload bandwidth
- A public IP or port forwarding to one of the server ports

## Quick start

1. Download the latest release zip from [Releases](https://github.com/ArmaField/ArmaField-Windows-Server/releases).
2. Unpack to any folder - e.g. `C:\ArmaField Server\`. This becomes your installation root.
3. Open a command prompt in that folder.
4. `armafield-server install` - downloads SteamCMD, installs Arma Reforger, seeds `launcher.toml` and `config.json` from templates.
5. Edit `launcher.toml`: set `network.public_address` to your public IP, review ports.
6. Edit `config.json`: change `game.passwordAdmin`, `rcon.password`, `game.name`, `game.admins`, etc.
7. (Recommended) From an **elevated** PowerShell: `armafield-server firewall add`.
8. `armafield-server run` - starts the server in the foreground. Ctrl+C stops it cleanly.

The first `install` downloads ~10 GB - expect 5–10 minutes.

## Running as a Windows Service

From an **elevated** PowerShell:

```powershell
armafield-server service install --auto
armafield-server service start
armafield-server service status        # current SCM state
armafield-server service stop
armafield-server service uninstall
```

The service runs under `LocalSystem`. The installer stores the install directory in the service's `ARMAFIELD_HOME` environment variable so paths resolve correctly from `C:\Windows\System32`.

Logs while running as a service land in `logs/launcher.log.YYYY-MM-DD` (JSON). The game server's own logs remain under `profile/logs/`.

## Configuration - `launcher.toml`

All sections are optional; defaults are listed. See `launcher.example.toml` for an annotated template.

| Field | Default | Meaning |
|---|---|---|
| `network.public_address` | `""` | Public IPv4 clients see. Empty → leave `config.json:publicAddress` untouched. |
| `network.game_port` | `2001` | UDP bind + public port. |
| `network.a2s_port` | `17777` | UDP A2S query port. |
| `network.rcon_port` | `19999` | UDP RCON port. |
| `arma.params` | `-maxFPS 120 -backendlog -nothrow` | Raw startup flags. [Full list](https://community.bistudio.com/wiki/Arma_Reforger:Startup_Parameters). |
| `steamcmd.check_interval_minutes` | `60` | Gap between SteamCMD validation runs. `0` = every launch. |
| `steamcmd.skip_install` | `false` | Skip SteamCMD entirely. |
| `steamcmd.app_id` | `"1874900"` | Arma Reforger dedicated-server AppID. |
| `firewall.rule_prefix` | `"ArmaField Server"` | Prefix for the three UDP rule names. |
| `firewall.auto_manage` | `false` | Add/remove rules on `run` / service start/stop. Requires admin / LocalSystem. |
| `service.name` | `"ArmaFieldServer"` | Service name under SCM. |
| `service.start_type` | `"auto"` | `auto` / `manual` / `disabled`. |

## Configuration - `config.json`

Arma Reforger server config, consumed verbatim from the [Bohemia wiki spec](https://community.bistudio.com/wiki/Arma_Reforger:Server_Config). The launcher **forces** these fields on each launch - your `config.json` values are ignored:

- `bindAddress`, `a2s.address`, `rcon.address` → `0.0.0.0`
- `bindPort`, `publicPort` → `network.game_port`
- `a2s.port` → `network.a2s_port`
- `rcon.port` → `network.rcon_port`
- `publicAddress` → `network.public_address` (only if non-empty)
- `game.scenarioId` → `MapSeeding.json` override (when present and valid)

The patched config is written to `state/runtime_config.json` before each launch; your original `config.json` is never modified.

## Windows Firewall

The launcher creates three UDP allow-rules named `ArmaField Server GAME`, `ArmaField Server A2S`, `ArmaField Server RCON` (prefix configurable).

```powershell
armafield-server firewall add        # admin - idempotent
armafield-server firewall remove     # admin
```

`add` is idempotent - it removes the three rules first, then adds them with the current port values. Run it after any port change in `launcher.toml` to sync.

If `launcher.toml:firewall.auto_manage = true`, the launcher adds/removes rules on start/stop automatically (requires admin for console mode; LocalSystem for service mode). Default: off.

## File layout

```
C:\ArmaField\
  armafield-server.exe
  launcher.toml           config.json
  launcher.example.toml   example_config.json
  README.md  LICENSE
  steamcmd/               (auto)
  server/                 (auto; managed by SteamCMD)
  profile/                (Arma Reforger server profile)
    profile/ArmaField/Systems/MapSeeding.json
    profile/ArmaField/Systems/BackendSettings.json   (mod backend - see section below)
    logs/                 (Arma Reforger's own log files)
  workshop/               (mods)
  logs/                   (launcher logs)
  state/                  (steamcmd.marker, runtime_config.json)
```

## MapSeeding rotation

After each mission the ArmaField mod writes the next scenario ID to `profile/profile/ArmaField/Systems/MapSeeding.json`. The server exits, the launcher's supervisor picks up the new value and launches the next mission automatically. Missing / invalid file → fallback to `config.json:game.scenarioId`.

## ArmaField mod: backend configuration

The ArmaField mod connects to an ArmaField backend for match statistics, player identity, and other cross-server features. The mod reads its backend settings from `profile/profile/ArmaField/Systems/BackendSettings.json`.

> **Why the double `profile/`?** `ArmaReforgerServer.exe` creates a `profile/` subdirectory inside whatever directory we pass via `-profile`, so the mod's files land at `profile/profile/ArmaField/Systems/` on the host - right next to `MapSeeding.json` written by the mod between missions.

Create (or edit) this file **before** starting the server. From PowerShell in the install directory:

```powershell
New-Item -ItemType Directory -Force -Path "profile\profile\ArmaField\Systems"
notepad profile\profile\ArmaField\Systems\BackendSettings.json
```

File content:

```json
{
    "ServerToken": "YOUR-SERVER-TOKEN",
    "BackendURL": "https://your.backend.url"
}
```

> **`BackendURL` MUST be HTTPS with a valid SSL certificate.** Arma Reforger rejects plain HTTP outright - `http://...` URLs will not work even for local testing. If you self-host the backend, put it behind a reverse proxy with a Let's Encrypt cert (Caddy, Traefik, Nginx + Certbot - any of them) before pointing the mod at it.

**Defaults** (test backend - use only for local development or testing, not for public play):

```json
{
    "ServerToken": "ARMAFIELD-TEST-TOKEN",
    "BackendURL": "https://test.armafield.gg"
}
```

For production you must **self-host** the backend - there is no free public backend to connect to. The open-source backend can be found at the [ArmaField BackEnd repository](https://github.com/ArmaField/ArmaField-BackEnd).

**Without a reachable backend the ArmaField mod does not function at all** - the spawn menu may fail to open and players will not be able to spawn into the match. A working HTTPS backend with a valid `ServerToken` is a hard requirement for the mod to run, not an optional add-on for stats.

The `profile/` directory persists across launcher upgrades and game updates - you only need to set `BackendSettings.json` up once per host. The launcher never touches this file.

## Building from source

```powershell
git clone https://github.com/ArmaField/ArmaField-Windows-Server.git
cd ArmaField-Windows-Server
cargo build --release
```

Binary at `target\release\armafield-server.exe`.

Run the tests:

```powershell
cargo test --all
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Clean shutdown |
| 1 | Config error (`launcher.toml` / `config.json`) |
| 2 | SteamCMD failed and game is not installed |
| 3 | Admin required but not elevated |
| 4 | Filesystem-fatal error |

## License

[MIT](./LICENSE) © ARMAFIELD