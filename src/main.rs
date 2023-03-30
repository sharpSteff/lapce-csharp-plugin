use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf, time::SystemTime,
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use lapce_plugin::{
    psp_types::{
        lsp_types::{request::Initialize, DocumentFilter, DocumentSelector, InitializeParams, Url},
        Request,
    },
    register_plugin, Http, LapcePlugin, VoltEnvironment, PLUGIN_RPC,
};
use serde_json::Value;
use zip::ZipArchive;
use wasi_experimental_http::Response;

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

    self::log(&mut file, "Reading installed omnisharp version");

    // check last omnisharp version
    let mut last_ver = ok!(fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(".omnisharp_ver"));
    let mut installed_version = String::new();
    ok!(last_ver.read_to_string(&mut installed_version));

    self::log(&mut file, &format!("Last installed version {}", installed_version));

    // Check for the latest release version
    let release_url = "https://api.github.com/repos/OmniSharp/omnisharp-roslyn/releases/latest?user-agent=Lapce";

    let mut resp = ok!(self::get(release_url));
    PLUGIN_RPC.stderr(&format!("STATUS_CODE: {:?}", resp.status_code));
    self::log(&mut file, &format!("Response Status: {}", resp.status_code));

    let body = ok!(resp.body_read_all());
    let raw_string = String::from_utf8(body).unwrap();

    // Parse the response body as JSON
    let json = serde_json::from_str::<serde_json::Value>(&raw_string).unwrap();
    let latest_version = json["tag_name"].as_str().unwrap();

    self::log(&mut file, &format!("Lastest version {}", latest_version));

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

    let mut server_path =  PathBuf::from("bin");

    // Compare versions and download if necessary
    if installed_version != latest_version {
        // Remove any existing OmniSharp binaries
        let _ = fs::remove_dir_all(server_path.clone());

        // Download and extract the latest release
        // https://github.com/OmniSharp/omnisharp-roslyn/releases/download/latest/omnisharp-osx-x64-net6.0.zip
        let download_url = format!(
            "https://github.com/OmniSharp/omnisharp-roslyn/releases/download/{}/{}",
            latest_version, zip_file
        );

        self::log(&mut file, &format!("Download file from {} to {}", download_url, zip_file));

        let mut resp = ok!(Http::get(&download_url));
        PLUGIN_RPC.stderr(&format!("STATUS_CODE: {:?}", resp.status_code));
        self::log(&mut file, &format!("Response Status: {}", resp.status_code));
        let body = ok!(resp.body_read_all());
        ok!(fs::write(&zip_file, body));

        match fs::create_dir_all(&server_path) {
            Ok(_) => {},
            Err(e) =>{
                self::log(&mut file, &format!("Failed to create dir {} {}", server_path.to_str().unwrap(), e));
                panic!("Unable to create dir");
            },
        }

        let mut zip = ok!(ZipArchive::new(ok!(File::open(&zip_file))));
        match zip.extract(&server_path) {
            Ok(()) => self::log(&mut file, "Zip file extracted successfully"),
            Err(e) => {
                self::log(&mut file, &format!("Failed to extract zip file: {}", e));
                panic!("Unable to extract zip file");
            },
        }

        ok!(fs::remove_file(&zip_file));
        ok!(last_ver.write_all(latest_version.as_bytes()));
    } else {
        self::log(&mut file, "No new version found!");
    }

    let volt_uri = ok!(VoltEnvironment::uri());
 
    server_path = match VoltEnvironment::operating_system().as_deref() {
        | Ok("windows") => server_path.join("OmniSharp.exe"),
        | _ => server_path.join("OmniSharp"),
    };

    let server_path_string = match server_path.to_str() {
        | Some(v) => v,
        | None => return Err(anyhow!("server_path.to_str() failed")),
    };

    let server_uri = ok!(ok!(Url::parse(&volt_uri)).join(server_path_string));
    self::log(&mut file, &format!("Starting OmniSharp from {}", server_uri));

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

fn get(url: &str) -> Result<Response> {
    let req = http::request::Builder::new()
        .method(http::Method::GET)
        .header("User-Agent", "Lapce")
        .uri(url)
        .body(None)?;
    let resp = wasi_experimental_http::request(req)?;
    Ok(resp)
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
