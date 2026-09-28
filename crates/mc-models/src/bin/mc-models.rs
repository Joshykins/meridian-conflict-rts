//! Builds meshes for another process: `mc-models REQUEST RESPONSE` reads a bincode
//! `mc_models::remote::Request` from the file REQUEST, builds every call's model in
//! parallel and writes the `Response` (one `Option<Model>` per call, in order) to
//! RESPONSE. A warm shot server runs it after a model edit instead of relinking the
//! game (`mc_models::remote`).

use std::process::ExitCode;

use mc_models::remote;

fn run() -> Result<(), String> {
    let usage = || "usage: mc-models REQUEST RESPONSE [--as MESH]".to_string();
    let mut args = std::env::args_os().skip(1);
    let (Some(request), Some(response)) = (args.next(), args.next()) else {
        return Err(usage());
    };
    // `--as MESH`: build every call with mesh MESH in place of its own (a design
    // variant, `tank_light~slim`), keeping the call's key on the result so the
    // renderer's per-model traits still find it.
    let variant = match (args.next(), args.next(), args.next()) {
        (None, None, None) => None,
        (Some(flag), Some(mesh), None) if flag == "--as" => {
            Some(mesh.into_string().map_err(|_| usage())?)
        }
        _ => return Err(usage()),
    };
    let bytes = std::fs::read(&request)
        .map_err(|e| format!("{}: {e}", std::path::Path::new(&request).display()))?;
    let calls = remote::decode_request(&bytes).map_err(|e| e.to_string())?;
    let models = match &variant {
        None => remote::build_all(&calls),
        Some(mesh) => {
            let as_variant: Vec<remote::Call> = calls
                .iter()
                .map(|c| remote::Call {
                    key: mesh.clone(),
                    ..c.clone()
                })
                .collect();
            let mut built = remote::build_all(&as_variant);
            if built.iter().all(Option::is_none) {
                return Err(format!("no model is made for mesh {mesh:?}"));
            }
            for (model, call) in built.iter_mut().zip(&calls) {
                if let Some(m) = model {
                    m.key = call.key.clone();
                }
            }
            built
        }
    };
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
