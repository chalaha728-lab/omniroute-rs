//! RunPod — OpenAI-compatible serverless GPU inference.
//! Endpoint: POST https://api.runpod.ai/v2/openai/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.runpod.ai/v2/openai";
const DEFAULT_MODELS: &[&str] = &[
    "meta-llama/Meta-Llama-3.1-405B-Instruct",
    "meta-llama/Meta-Llama-3.1-70B-Instruct",
    "mistralai/Mixtral-8x22B-Instruct-v0.1",
];

pub type RunPod = OpenAI;

pub fn new(api_key: Option<String>) -> RunPod {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::RunPod, "runpod", DEFAULT_MODELS)
}
