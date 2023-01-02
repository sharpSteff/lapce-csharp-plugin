# Lapce plugin for C# (`OmniSharp`)

## Prerequisites

Install `OmniSharp` and `dotnet-sdk`
Server needs to be included in `PATH` environment variable

```shell
curl -LO https://github.com/OmniSharp/omnisharp-roslyn/releases/download/latest/omnisharp-<OS>-net.6.0.<tar.gz/zip>

tar -xzf omnisharp-<OS>-net.6.0.<tar.gz/zip> <PathToInstall>

```

## Available configuration

```toml
[lapce-csharp.volt]
serverPath = "<custom executable>"
serverArgs = ["--languageserver"] # --languageserver is required for OmniSharp
```
