//! IBM Watsonx — OpenAI-compatible via the watsonx.ai gateway.
//! Endpoint: POST https://us-south.ml.cloud.ibm.com/ml/v1/openai/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://us-south.ml.cloud.ibm.com/ml/v1/openai";
const DEFAULT_MODELS: &[&str] = &[
    "ibm/granite-3-8b-instruct",
    "ibm/granite-3-2b-instruct",
    "ibm/granite-13b-chat-v2",
    "meta-llama/llama-3-3-70b-instruct",
    "meta-llama/llama-3-1-8b-instruct",
    "mistralai/mistral-large",
];

pub type Watsonx = OpenAI;

pub fn new(api_key: Option<String>) -> Watsonx {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Watsonx, "ibm", DEFAULT_MODELS)
}
