//! SiliconFlow — OpenAI-compatible.
//! Endpoint: POST https://api.siliconflow.cn/v1/chat/completions

use super::openai::OpenAI;
use crate::models::provider::ProviderId;

const BASE_URL: &str = "https://api.siliconflow.cn/v1";
const DEFAULT_MODELS: &[&str] = &[
    "deepseek-ai/DeepSeek-V3",
    "meta-llama/Meta-Llama-3.1-405B-Instruct",
    "Qwen/Qwen2.5-72B-Instruct",
    "Pro/Qwen/Qwen2.5-72B-Instruct",
];

pub type SiliconFlow = OpenAI;

pub fn new(api_key: Option<String>) -> SiliconFlow {
    OpenAI::with_base_url(api_key, BASE_URL, ProviderId::SiliconFlow, "siliconflow", DEFAULT_MODELS)
}
