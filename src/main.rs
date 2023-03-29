use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write, BufWriter},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use lapce_plugin::{
    psp_types::{
        lsp_types::{request::Initialize, DocumentFilter, DocumentSelector, InitializeParams, Url},
        Request,
    },
    register_plugin, Http, LapcePlugin, VoltEnvironment, PLUGIN_RPC,
};
use serde_json::Value;
use zip::ZipArchive;

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
        pattern: Some(string!("**/*.{cs,csx}")),
        scheme: None,
    }];
    let mut server_args = vec![string!("--languageserver")];

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
    }

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("csharp_plugin.log")
        .expect("failed to open file");

    writeln!(file, "Reading installed omnisharp version").unwrap();

    // check last omnisharp version
    let mut last_ver = ok!(fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(".omnisharp_ver"));
    let mut installed_version = String::new();
    ok!(last_ver.read_to_string(&mut installed_version));

    writeln!(file, "Last installed version {}", installed_version).unwrap();

    // Check for the latest release version
    let release_url = "https://api.github.com/repos/OmniSharp/omnisharp-roslyn/releases/latest?user-agent=Lapce";

    let mut resp = ok!(Http::get(release_url));
    PLUGIN_RPC.stderr(&format!("STATUS_CODE: {:?}", resp.status_code));
    writeln!(file, "Check on github Status: {}", resp.status_code).unwrap();
    let body = ok!(resp.body_read_all());
    let raw_string = String::from_utf8(body).unwrap();

    writeln!(file, "Github Response: {}", raw_string).unwrap();

    // Parse the response body as JSON
    let json = serde_json::from_str::<serde_json::Value>(&raw_string).unwrap();
    let latest_version = json["tag_name"].as_str().unwrap();

    let architecture = match VoltEnvironment::architecture().as_deref() {
        Ok("x86_64") => "x64",
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => return Ok(()),
    };

    // linux omnisharp-linux-arm64-net6.0.zip
    // macos omnisharp-osx-x64-net6.0.zip
    let zip_file = match VoltEnvironment::operating_system().as_deref() {
        Ok("macos") => format!("omnisharp-osx-{architecture}-net6.0.zip"),
        Ok("linux") => format!("omnisharp-linux-{architecture}-net6.0.zip"),
        Ok("windows") => format!("omnisharp-win-{architecture}-net6.0.zip"),
        Ok(v) => return Err(anyhow!("Unsupported OS: {}", v)),
        Err(e) => return Err(anyhow!("Error OS: {}", e)),
    };

    let zip_path = PathBuf::from(zip_file.clone());
    let server_path = zip_path.join("OmniSharp");

    // Compare versions and download if necessary
    if installed_version != latest_version {
        // Remove any existing OmniSharp binaries
        let _ = fs::remove_dir_all(server_path.clone());

        // Download and extract the latest release
        // https://github.com/OmniSharp/omnisharp-roslyn/releases/download/latest/omnisharp-osx-x64-net6.0.zip
        let download_url = format!(
            "https://github.com/OmniSharp/omnisharp-roslyn/releases/download/{}/omnisharp-{}",
            latest_version, zip_file
        );

        let mut resp = ok!(Http::get(&download_url));
        PLUGIN_RPC.stderr(&format!("STATUS_CODE: {:?}", resp.status_code));
        let body = ok!(resp.body_read_all());
        ok!(fs::write(&zip_file, body));

        let mut zip = ok!(ZipArchive::new(ok!(File::open(&zip_file))));

        for i in 0..zip.len() {
            let mut file = ok!(zip.by_index(i));
            let outpath = match file.enclosed_name() {
                Some(path) => path.to_owned(),
                None => continue,
            };

            if (*file.name()).ends_with('/') {
                ok!(fs::create_dir_all(&outpath));
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        ok!(fs::create_dir_all(p));
                    }
                }
                let mut outfile = ok!(File::create(&outpath));
                ok!(io::copy(&mut file, &mut outfile));
            }
        }

        ok!(fs::remove_file(&zip_file));
        ok!(last_ver.write_all(latest_version.as_bytes()));
    }

    let volt_uri = ok!(VoltEnvironment::uri());
    let server_path_string = match server_path.to_str() {
        | Some(v) => v,
        | None => return Err(anyhow!("server_path.to_str() failed")),
    };
    let server_uri = ok!(ok!(Url::parse(&volt_uri)).join(server_path_string));

    PLUGIN_RPC.start_lsp(
        server_uri,
        server_args,
        document_selector,
        params.initialization_options,
    );

    Ok(())
}

impl LapcePlugin for State {
    fn handle_request(&mut self, _id: u64, method: String, params: Value) {
        #[allow(clippy::single_match)]
        match method.as_str() {
            Initialize::METHOD => {
                let params: InitializeParams = serde_json::from_value(params).unwrap();
                if let Err(e) = initialize(params) {
                    PLUGIN_RPC.stderr(&format!("plugin returned with error: {e}"))
                }
            }
            _ => {}
        }
    }
}
