//! Builds meshes for another process: `mc-models REQUEST RESPONSE` reads a bincode
//! `mc_models::remote::Request` from the file REQUEST, builds every call's model in
//! parallel and writes the `Response` (one `Option<Model>` per call, in order) to
//! RESPONSE. A warm shot server runs it after a model edit instead of relinking the
//! game (`mc_models::remote`).

use std::process::ExitCode;

use mc_models::remote;

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let (Some(request), Some(response), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: mc-models REQUEST RESPONSE".to_string());
    };
    let bytes = std::fs::read(&request)
        .map_err(|e| format!("{}: {e}", std::path::Path::new(&request).display()))?;
    let calls = remote::decode_request(&bytes).map_err(|e| e.to_string())?;
    let models = remote::build_all(&calls);
    let out = remote::encode_response(&models).map_err(|e| e.to_string())?;
    std::fs::write(&response, out)
        .map_err(|e| format!("{}: {e}", std::path::Path::new(&response).display()))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mc-models: {e}");
            ExitCode::FAILURE
        }
    }
}
