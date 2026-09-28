//! Building meshes out of process. After a model edit, a warm headless shot server
//! need not relink the whole game to see it: it asks the small `mc-models` program
//! (built from this crate alone, in seconds) to rebuild the meshes it draws, and
//! swaps them in. The two sides talk in files of bincode: a [`Request`] of
//! [`Call`]s in, a [`Response`] with one `Option<Model>` per call, in order, out.
//!
//! Both files come from outside the reading process, so they are decoded with a
//! size limit (CLAUDE.md section 7).

use bincode::Options;
use serde::{Deserialize, Serialize};

use crate::{build_model, build_model_fitted, Model};

/// One mesh to build.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub key: String,
    /// The blueprint's collision radius and height, and its tech level
    /// ([`build_model_fitted`]). Unused for a prop.
    pub radius: f32,
    pub height: f32,
    pub tech: u8,
    /// Refit module keys in look-bit order.
    pub modules: Vec<String>,
    /// A prop (tree, rock, Precursor piece): built at its authored size
    /// ([`build_model`]).
    pub prop: bool,
}

/// What a shot server asks for.
pub type Request = Vec<Call>;
/// One entry per call, in the request's order; None for a key no model is made for.
pub type Response = Vec<Option<Model>>;

/// The largest request read: a few hundred keys take a few tens of kilobytes.
pub const MAX_REQUEST_BYTES: u64 = 16 << 20;
/// The largest response read: every mesh the game draws, at every level of detail,
/// comes to well under this.
pub const MAX_RESPONSE_BYTES: u64 = 2 << 30;

/// Why a request or response could not be read or written.
#[derive(Debug)]
pub enum RemoteError {
    /// The bytes are not a request or response, or are longer than their limit.
    Codec(bincode::Error),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteError::Codec(e) => write!(f, "mesh request/response: {e}"),
        }
    }
}

impl std::error::Error for RemoteError {}

impl From<bincode::Error> for RemoteError {
    fn from(e: bincode::Error) -> Self {
        RemoteError::Codec(e)
    }
}

/// The encoding both sides use: bincode's fixed-width integers (as
/// `bincode::serialize` writes them), reading at most `limit` bytes.
fn codec(limit: u64) -> impl Options {
    bincode::options().with_fixint_encoding().with_limit(limit)
}

pub fn encode_request(request: &[Call]) -> Result<Vec<u8>, RemoteError> {
    Ok(codec(MAX_REQUEST_BYTES).serialize(request)?)
}

pub fn decode_request(bytes: &[u8]) -> Result<Request, RemoteError> {
    Ok(codec(MAX_REQUEST_BYTES).deserialize(bytes)?)
}

pub fn encode_response(response: &[Option<Model>]) -> Result<Vec<u8>, RemoteError> {
    Ok(codec(MAX_RESPONSE_BYTES).serialize(response)?)
}

pub fn decode_response(bytes: &[u8]) -> Result<Response, RemoteError> {
    Ok(codec(MAX_RESPONSE_BYTES).deserialize(bytes)?)
}

/// Builds one call's mesh.
pub fn build(call: &Call) -> Option<Model> {
    if call.prop {
        return build_model(&call.key);
    }
    let modules: Vec<&str> = call.modules.iter().map(String::as_str).collect();
    build_model_fitted(&call.key, call.radius, call.height, call.tech, &modules)
}

/// Builds every call's mesh on a handful of threads, answers in the request's order.
/// Calls are dealt round-robin, so the heavy models (titans, capital ships) spread
/// over the workers rather than landing on one.
pub fn build_all(request: &[Call]) -> Response {
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8)
        .min(request.len().max(1));
    let mut out: Response = vec![None; request.len()];
    let built: Vec<Vec<(usize, Option<Model>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                scope.spawn(move || {
                    request
                        .iter()
                        .enumerate()
                        .skip(w)
                        .step_by(workers)
                        .map(|(i, call)| (i, build(call)))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    for (i, model) in built.into_iter().flatten() {
        out[i] = model;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(key: &str) -> Call {
        let (radius, height) = crate::authored_size(key).expect("a known key");
        Call {
            key: key.to_string(),
            radius,
            height,
            tech: 1,
            modules: Vec::new(),
            prop: false,
        }
    }

    #[test]
    fn a_built_model_survives_bincode() {
        let key = "tank_light";
        let model = build_model(key).expect("the tank has a model");
        let bytes = encode_response(&[Some(model.clone())]).unwrap();
        let back = decode_response(&bytes).unwrap();
        assert_eq!(back.len(), 1);
        let back = back[0].as_ref().expect("the model came back");
        assert_eq!(back.key, key);
        assert_eq!(
            back.lods[0].vertices.len(),
            model.lods[0].vertices.len(),
            "LOD0 vertex count"
        );
        assert_eq!(*back, model);
    }

    #[test]
    fn build_all_answers_in_order() {
        let mut request: Request = ["tank_light", "commander", "no_such_model", "tank_light"]
            .iter()
            .map(|k| match crate::authored_size(k) {
                Some(_) => unit(k),
                None => Call {
                    key: k.to_string(),
                    radius: 1.0,
                    height: 1.0,
                    tech: 1,
                    modules: Vec::new(),
                    prop: false,
                },
            })
            .collect();
        request.push(Call {
            key: "tree_pine".to_string(),
            radius: 0.0,
            height: 0.0,
            tech: 0,
            modules: Vec::new(),
            prop: true,
        });
        let request = decode_request(&encode_request(&request).unwrap()).unwrap();
        let response = build_all(&request);
        let keys: Vec<Option<&str>> = response
            .iter()
            .map(|m| m.as_ref().map(|m| m.key.as_str()))
            .collect();
        assert_eq!(
            keys,
            [
                Some("tank_light"),
                Some("commander"),
                None,
                Some("tank_light"),
                Some("tree_pine")
            ]
        );
        assert_eq!(response[0], build(&request[0]));
    }

    #[test]
    fn corrupt_bytes_are_an_error_not_a_panic() {
        assert!(decode_request(&[0xff; 16]).is_err());
        assert!(decode_response(&[0xff; 16]).is_err());
        assert!(decode_request(&[]).is_err());
    }
}
