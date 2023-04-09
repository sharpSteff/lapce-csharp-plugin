# Lapce plugin for C# (`csharp-ls`)

C# (csharp) Plugin for Lapce powered by [csharp-ls](https://github.com/razzmatazz/csharp-language-server) an alternative, fast LSP to omnisharp.

## Prerequisites

Install `csharp-ls` and `dotnet-sdk` for .NET 7

```shell
dotnet tool install --global csharp-ls
```

## Advanced configuration

```toml
[config."csharp.solution"]
default = ""
description = "Path to the solution file"

[config."csharp.loglevel"]
default = "error"
description = "the default log level"

[config."csharp.plugindebug"]
default = "false"
description = "enables detailed logging about the plugin-lsp interaction"
```
