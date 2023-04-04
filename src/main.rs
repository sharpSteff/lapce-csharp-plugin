use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use lapce_plugin::{
    psp_types::{
        lsp_types::{
            request::Initialize, DocumentFilter, DocumentSelector, InitializeParams,
            PublishDiagnosticsClientCapabilities, Url,
        },
        Request,
    },
    register_plugin, LapcePlugin, VoltEnvironment, PLUGIN_RPC,
};
use serde_json::Value;

#[derive(Default)]
struct State {}

register_plugin!(State);

macro_rules! string {
    ( $x:expr ) => {
        String::from($x)
    };
}

macro_rules! ok {
    ( $x:expr ) => {
        match ($x) {
            Ok(v) => v,
            Err(e) => return Err(anyhow!(e)),
        }
    };
}
fn initialize(params: InitializeParams) -> Result<()> {
    let document_selector: DocumentSelector = vec![DocumentFilter {
        language: Some(string!("csharp")),
        pattern: Some(string!("**/*.cs")),
        scheme: None,
    }];

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("csharp_plugin.log")
        .expect("failed to open file");

    let mut server_args = vec![];

    // // look for solution file
    // if let Some(workspace) = params.workspace_folders.as_ref() {
    //     workspace.
    // }

    let mut log_level = "warning";
    if let Some(options) = params.initialization_options.as_ref() {
        if let Some(volt) = options.get("volt") {
            if let Some(args) = volt.get("serverArgs") {
                if let Some(args) = args.as_array() {
                    if !args.is_empty() {
                        server_args = vec![];
                    }
                    for arg in args {
                        if let Some(arg) = arg.as_str() {
                            server_args.push(arg.to_string());
                        }
                    }
                }
            }

            if let Some(server_path) = volt.get("serverPath") {
                if let Some(server_path) = server_path.as_str() {
                    if !server_path.is_empty() {
                        let server_uri = ok!(Url::parse(&format!("urn:{}", server_path)));
                        PLUGIN_RPC.start_lsp(
                            server_uri,
                            server_args,
                            document_selector,
                            params.initialization_options,
                        );
                        return Ok(());
                    }
                }
            }
        }

        if let Some(csharp) = options.get("csharp") {
            if let Some(solution) = csharp.get("solution") {
                if let Some(arg) = solution.as_str() {
                    let solution_arg = format!("solution {arg}");
                    server_args.push(solution_arg);
                }
            }
        }

        if let Some(csharp) = options.get("csharp") {
            if let Some(solution) = csharp.get("loglevel") {
                if let Some(arg) = solution.as_str() {
                    log_level = arg;
                }
            }
        }
    }

    server_args.push("--loglevel".to_owned());
    server_args.push(log_level.to_owned());
    let mut server_path = PathBuf::from("bin_csharpserver");

    let volt_uri = ok!(VoltEnvironment::uri());

    server_path = match VoltEnvironment::operating_system().as_deref() {
        Ok("windows") => server_path.join("CSharpLanguageServer.exe"),
        _ => server_path.join("CSharpLanguageServer"),
    };

    let server_path_string = match server_path.to_str() {
        Some(v) => v,
        None => return Err(anyhow!("server_path.to_str() failed")),
    };

    let server_uri = ok!(ok!(Url::parse(&volt_uri)).join(server_path_string));
    let args_string = server_args.join(" ");
    self::log(
        &mut file,
        &format!(
            "Starting CSharpLanguageServer from {} with args {}",
            server_uri, args_string
        ),
    );

    // let server_uri = match VoltEnvironment::operating_system().as_deref() {
    //     | Ok("windows") => ok!(Url::parse("urn:csharp-ls")),
    //     | _ => ok!(Url::parse("urn:csharp-ls")),
    // };

    let args_string = server_args.join(" ");
    self::log(
        &mut file,
        &format!(
            "Starting csharp-ls from {} with args {}",
            server_uri, args_string
        ),
    );

    PLUGIN_RPC.start_lsp(
        server_uri,
        server_args,
        document_selector,
        params.initialization_options,
    );

    Ok(())
}

fn log(file: &mut File, message: &str) {
    let timestamp = DateTime::<Utc>::from(SystemTime::now());
    let timestamp_str = timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
    writeln!(file, "[{}] {}", timestamp_str, message).unwrap();
}

impl LapcePlugin for State {
    
    fn handle_request(&mut self, _id: u64, method: String, params: Value) {
        #[allow(clippy::single_match)]
        match method.as_str() {
            Initialize::METHOD => {
                let mut params: InitializeParams = serde_json::from_value(params).unwrap();

                if let Some(td) = &mut params.capabilities.text_document {
                    if td.publish_diagnostics.is_none() {
                        td.publish_diagnostics = Some(PublishDiagnosticsClientCapabilities::default());
                    }
                }

                if let Err(e) = initialize(params) {
                    PLUGIN_RPC.stderr(&format!("plugin returned with error: {e}"))
                }
            }
            _ => {}
        }
    }
}
