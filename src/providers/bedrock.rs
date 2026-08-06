//! AWS Bedrock — OpenAI-compatible via the Bedrock API Gateway.
//! Endpoint: POST https://bedrock-runtime.us-east-1.amazonaws.com/openai/v1/chat/completions
//! Auth: AWS SigV4 (simplified here — pass AWS_BEDROCK_API_KEY for the OpenAI-compatible gateway)
//!
//! Note: native Bedrock uses invoke-api with provider-specific request formats
//! (Anthropic, Meta, etc.). The OpenAI-compatible gateway at /openai/v1/* is
//! the simplest path — requires `bedrock:OpenAICompatible` enabled on your account.

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://bedrock-runtime.us-east-1.amazonaws.com/openai/v1";
const DEFAULT_MODELS: &[&str] = &[
    "anthropic.claude-3-5-sonnet-20241022-v2:0",
    "anthropic.claude-3-5-haiku-20241022-v1:0",
    "anthropic.claude-3-opus-20240229-v1:0",
    "meta.llama3-3-70b-instruct-v1:0",
    "meta.llama3-1-8b-instruct-v1:0",
    "amazon.nova-pro-v1:0",
    "amazon.nova-lite-v1:0",
    "amazon.nova-micro-v1:0",
];

pub type Bedrock = OpenAI;

pub fn new(api_key: Option<String>) -> Bedrock {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::Bedrock, "aws", DEFAULT_MODELS)
}
