use anyhow::Error;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Response, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use clap::{Parser, ValueEnum};
use host::types::{EpochChangeData, InclusionData, ProvingMode, Request};
use host::{epoch_change, inclusion};
use risc0_zkvm::{default_prover, ProverOpts, VerifierContext};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::spawn_blocking;
use tracing::{error, info};

use aptos_guests::INCLUSION_ELF;
use aptos_guests::{EPOCH_CHANGE_ELF, EPOCH_CHANGE_ID, INCLUSION_ID};


#[derive(Parser)]
struct Cli {
    /// Address of this server. E.g. 127.0.0.1:4321
    #[arg(short, long)]
    addr: String,

    /// Address of the secondary server. E.g. 127.0.0.1:4321
    #[arg(short, long)]
    snd_addr: Option<String>,
    #[arg(short, long)]
    mode: Mode,
}


#[derive(ValueEnum, Clone, Debug, Eq, PartialEq)]
enum Mode {
    Single,
    Split,
}

#[derive(Clone)]
struct ServerState {
    snd_addr: Arc<Option<String>>,
    mode: Mode,
    active_requests: Arc<AtomicUsize>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli {
        addr,
        snd_addr,
        mode,
    } = Cli::parse();

    if mode == Mode::Split && snd_addr.is_none() {
        return Err(Error::msg(
            "Secondary server address is required in split mode",
        ));
    }

    let state = ServerState {
        snd_addr: Arc::new(snd_addr),
        mode,
        active_requests: Arc::new(AtomicUsize::new(0)),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/inclusion/proof", post(inclusion_proof))
        .route("/epoch/proof", post(epoch_proof))
        .route("/epoch/verify", post(epoch_verify))
        .route("/inclusion/verify", post(inclusion_verify))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            count_requests_middleware,
        ))
        .with_state(state);

    info!("Server running on {}", addr);

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

async fn ready_check(State(state): State<ServerState>) -> impl IntoResponse {
    let active_requests = state.active_requests.load(Ordering::SeqCst);
    if active_requests > 0 {
        StatusCode::CONFLICT
    } else {
        StatusCode::OK
    }
}

async fn inclusion_proof(
    State(state): State<ServerState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, StatusCode> {
    let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let res = bcs::from_bytes::<Request>(&bytes);

    if let Err(err) = res {
        error!("Failed to deserialize request object: {err}");
        return Err(StatusCode::BAD_REQUEST);
    }

    let request = res.unwrap();

    let Request::ProveInclusion(boxed) = request else {
        error!("Invalid request type");
        return Err(StatusCode::BAD_REQUEST);
    };
    let res = {
        info!("Start proving");

        let (proof_type, inclusion_data) = boxed.as_ref();
        let InclusionData {
            sparse_merkle_proof_assets,
            transaction_proof_assets,
            validator_verifier_assets,
        } = inclusion_data;
        let env = inclusion::generate_stdin(
            sparse_merkle_proof_assets,
            transaction_proof_assets,
            validator_verifier_assets,
        );

        let prover_client = Arc::new(Mutex::new(default_prover()));
        let proof_handle = if proof_type == &ProvingMode::SNARK {
            let prover_client = prover_client.lock().unwrap();
            prover_client.prove_with_ctx(
                env,
                &VerifierContext::default(),
                INCLUSION_ELF,
                &ProverOpts::groth16(),
            )
        } else {
            let prover_client = prover_client.lock().unwrap();
            prover_client.prove(env, INCLUSION_ELF)
        };


        let proof = proof_handle
            .map_err(|_| {
                error!("Failed to handle generate inclusion proof task");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        info!("Proof generated. Serializing");
        bcs::to_bytes(&proof.receipt).map_err(|err| {
            error!("Failed to serialize epoch change proof: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(res))
        .map_err(|err| {
            error!("Could not construct response for client: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

async fn inclusion_verify(
    State(_state): State<ServerState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, StatusCode> {
    let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let res = bcs::from_bytes::<Request>(&bytes);

    if let Err(err) = res {
        error!("Failed to deserialize request object: {err}");
        return Err(StatusCode::BAD_REQUEST);
    }

    let request = res.unwrap();

    let Request::VerifyInclusion(proof) = request else {
        error!("Invalid request type");
        return Err(StatusCode::BAD_REQUEST);
    };
    let res = {
        info!("Start verifying inclusion proof");

        let is_valid = proof
            .verify(INCLUSION_ID)
            .is_ok();

        info!("Inclusion verification result: {}", is_valid);

        bcs::to_bytes(&is_valid).map_err(|_| {
            error!("Failed to serialize inclusion verification result");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(res))
        .map_err(|err| {
            error!("Could not construct response for client: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

async fn epoch_proof(
    State(state): State<ServerState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, StatusCode> {
    let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let res = bcs::from_bytes::<Request>(&bytes);

    if let Err(err) = res {
        error!("Failed to deserialize request object: {err}");
        return Err(StatusCode::BAD_REQUEST);
    }

    let request = res.unwrap();

    let Request::ProveEpochChange(boxed) = request else {
        error!("Invalid request type");
        return Err(StatusCode::BAD_REQUEST);
    };
    let res = {
        match state.mode {
            Mode::Single => {
                let (proof_type, epoch_change_data) = boxed.as_ref();


                let EpochChangeData {
                    trusted_state,
                    epoch_change_proof,
                } = epoch_change_data;

                let env = epoch_change::generate_stdin(trusted_state, epoch_change_proof);
                info!("Start proving epoch change");

                let prover_client = Arc::new(Mutex::new(default_prover()));
                let proof_handle = if proof_type == &ProvingMode::SNARK {
                    let prover_client = prover_client.lock().unwrap();
                    prover_client.prove_with_ctx(
                        env,
                        &VerifierContext::default(),
                        EPOCH_CHANGE_ELF,
                        &ProverOpts::groth16(),
                    )
                } else {
                    let prover_client = prover_client.lock().unwrap();
                    prover_client.prove(env, EPOCH_CHANGE_ELF)
                };

                let proof = proof_handle
                    .map_err(|_| {
                        error!("Failed to handle generate epoch change proof task");
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                info!("Epoch change proof generated. Serializing");
                bcs::to_bytes(&proof.receipt).map_err(|err| {
                    error!("Failed to serialize epoch change proof: {err}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })
            }
            Mode::Split => {
                let snd_addr = state.snd_addr.as_ref().clone().unwrap();
                forward_request(bytes.to_vec(), &snd_addr).await
            }
        }
    }?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(res))
        .map_err(|err| {
            error!("Could not construct response for client: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

async fn epoch_verify(
    State(_state): State<ServerState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, StatusCode> {
    info!("Start verifying epoch change proof");

    let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let res = bcs::from_bytes::<Request>(&bytes);

    if let Err(err) = res {
        error!("Failed to deserialize request object: {err}");
        return Err(StatusCode::BAD_REQUEST);
    }

    let request = res.unwrap();

    let Request::VerifyEpochChange(proof) = request else {
        error!("Invalid request type");
        return Err(StatusCode::BAD_REQUEST);
    };
    let res = {
        let is_valid = proof.verify(EPOCH_CHANGE_ID).is_ok();

        info!("Epoch change verification result: {}", is_valid);

        bcs::to_bytes(&is_valid).map_err(|_| {
            error!("Failed to serialize epoch change verification result");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(res))
        .map_err(|err| {
            error!("Could not construct response for client: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

async fn forward_request(
    secondary_request_bytes: Vec<u8>,
    snd_addr: &str,
) -> Result<Vec<u8>, StatusCode> {
    info!("Connecting to the secondary server");
    let client = reqwest::Client::new();
    info!("Sending secondary request");
    let res_bytes = client
        .post(format!("http://{}/epoch/proof", snd_addr))
        .body(secondary_request_bytes)
        .send()
        .await
        .map_err(|err| {
            error!("Failed to send request to secondary server: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .bytes()
        .await
        .map_err(|err| {
            error!("Failed to receive response from secondary server: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Response received. Sending it to the client");

    Ok(res_bytes.to_vec())
}

async fn count_requests_middleware(
    State(state): State<ServerState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    let is_ready = req.uri().path() != "/ready";
    // Check if the request is for the ready endpoint.
    if is_ready {
        // Increment the active requests counter.
        state.active_requests.fetch_add(1, Ordering::SeqCst);
    }

    // Proceed with the request.
    let response = next.run(req).await;

    // Decrement the active requests counter if not a ready check.
    if is_ready {
        state.active_requests.fetch_sub(1, Ordering::SeqCst);
    }

    Ok(response)
}
