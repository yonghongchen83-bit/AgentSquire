//! Real local text embedding for Squire's `tool-token-registry` semantic
//! vector search, replacing the previous deterministic bag-of-words hash.
//!
//! Uses `fastembed`'s `BGESmallENV15` model (384-dim), a small CPU-only ONNX
//! model. The model is downloaded from Hugging Face on first use (a few tens
//! of MB) and cached under fastembed's default cache dir; inference thereafter
//! is offline and ms-scale.
//!
//! Design points:
//! - The model is a process-wide, lazily-initialized singleton behind a
//!   `Mutex` (inference takes `&mut self`, so it must be serialized).
//! - Initialization (download + ONNX session build) is blocking and can be
//!   slow on first run. `embed_text` runs it under `tokio::task::block_in_place`
//!   when called from within a multi-threaded tokio runtime, so it doesn't
//!   starve the async worker; outside a runtime it just blocks directly.
//! - If the model fails to initialize (offline first run, download failure,
//!   ONNX Runtime link/load issue), we log a warning ONCE and permanently fall
//!   back to the old bag-of-words hash so the app still functions (non-semantic
//!   ranking, but functional). The fallback also emits `EMBED_DIM` floats so
//!   the LanceDB schema stays consistent.
//!
//! Callers use `embed_text(&str) -> Vec<f32>`, the same signature the old
//! `squire_lancedb::embed_text` exposed, so call sites are unchanged.

use std::sync::{Mutex, OnceLock};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

/// Embedding dimensionality. `BGESmallENV15` produces 384-dim vectors; the
/// fallback hash pads/hashes into the same width so the fixed-size-list schema
/// column is always `EMBED_DIM` wide regardless of which path produced a row.
pub const EMBED_DIM: usize = 384;

/// Process-wide embedder state, initialized exactly once on first `embed_text`.
///
/// `Ok(model)` = real semantic embeddings; `Err(())` = init failed, use the
/// hash fallback forever. The `Mutex` both guards `&mut self` inference and
/// makes init happen-once.
static EMBEDDER: OnceLock<Result<Mutex<TextEmbedding>, ()>> = OnceLock::new();

/// Blocking model construction. Downloads the model on first run.
fn init_model() -> Result<Mutex<TextEmbedding>, ()> {
    match TextEmbedding::try_new(
        TextInitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(false),
    ) {
        Ok(model) => {
            log::info!(
                "Squire embedding: initialized fastembed BGESmallENV15 ({}-dim) for semantic search",
                EMBED_DIM
            );
            Ok(Mutex::new(model))
        }
        Err(e) => {
            log::warn!(
                "Squire embedding: failed to initialize fastembed model ({e}); \
                 falling back to non-semantic bag-of-words hash embeddings. \
                 Vector search will still work but ranking will be lexical only."
            );
            Err(())
        }
    }
}

/// Runs `init_model` without starving the async runtime. When we're inside a
/// multi-threaded tokio runtime, `block_in_place` moves this blocking work off
/// the async path; otherwise (tests, non-tokio contexts) we call it directly.
fn init_model_runtime_aware() -> Result<Mutex<TextEmbedding>, ()> {
    match tokio::runtime::Handle::try_current() {
        Ok(_) => tokio::task::block_in_place(init_model),
        Err(_) => init_model(),
    }
}

/// Embed `text` into an `EMBED_DIM`-length vector.
///
/// Returns a semantic embedding when the model is available, otherwise the
/// deterministic hash fallback. Never panics — a model inference error also
/// degrades to the fallback for that call.
///
// TODO: inference holds a `Mutex` and is CPU-bound; for higher throughput move
// the `.embed()` call onto `tokio::task::spawn_blocking`. Fine as-is for the
// small model / low call volume here (ms-scale, one row at a time).
pub fn embed_text(text: &str) -> Vec<f32> {
    match EMBEDDER.get_or_init(init_model_runtime_aware) {
        Ok(model_mutex) => {
            let result = {
                let mut model = model_mutex.lock().unwrap_or_else(|p| p.into_inner());
                model.embed(vec![text], None)
            };
            match result {
                Ok(mut vecs) if !vecs.is_empty() => {
                    let mut v = vecs.swap_remove(0);
                    // Defensive: guarantee the schema width even if a model
                    // ever returns an unexpected length.
                    v.resize(EMBED_DIM, 0.0);
                    v
                }
                Ok(_) => fallback_embed(text),
                Err(e) => {
                    log::warn!("Squire embedding: inference failed ({e}); using hash fallback for this input");
                    fallback_embed(text)
                }
            }
        }
        Err(()) => fallback_embed(text),
    }
}

/// Which embedding backend is active for this process.
///
/// Returns `"bge-small-384"` when the real fastembed model initialized
/// successfully, or `"fallback-hash"` when we're using the bag-of-words hash
/// fallback (offline first run / download failure / not yet initialized).
///
/// This is a pure observation accessor for tracing/debug: it does NOT force
/// initialization. If `embed_text` has not been called yet the embedder state
/// is unknown, and we report `"fallback-hash"` (the conservative, always-safe
/// answer) rather than triggering a blocking model download just to answer a
/// trace question. Once `embed_text` has run at least once, this reflects the
/// real, settled backend for the rest of the process lifetime.
pub fn active_backend() -> &'static str {
    match EMBEDDER.get() {
        Some(Ok(_)) => "bge-small-384",
        Some(Err(())) => "fallback-hash",
        // Not yet initialized — don't force a download just to answer.
        None => "fallback-hash",
    }
}

/// Deterministic hash-based bag-of-words embedding — the original, dependency
/// free fallback. Not semantically meaningful, but stable and always available.
/// Produces an `EMBED_DIM`-wide, L2-normalized vector so it stays schema
/// compatible with the real embeddings.
pub fn fallback_embed(text: &str) -> Vec<f32> {
    let mut vec = vec![0f32; EMBED_DIM];
    for token in text.to_lowercase().split_whitespace() {
        let mut hash: u64 = 1469598103934665603; // FNV offset basis
        for byte in token.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211); // FNV prime
        }
        let idx = (hash as usize) % EMBED_DIM;
        vec[idx] += 1.0;
    }
    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
    vec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_embed_dim_wide_and_normalized() {
        let v = fallback_embed("hello world");
        assert_eq!(v.len(), EMBED_DIM);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn embed_text_returns_embed_dim_vector() {
        // Works whether or not the model downloads: either the real 384-dim
        // embedding or the 384-dim fallback.
        let v = embed_text("rust programming language");
        assert_eq!(v.len(), EMBED_DIM);
    }
}
