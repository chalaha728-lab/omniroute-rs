//! Provider domain types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderId {
    OpenAI,
    Anthropic,
    Gemini,
    DeepSeek,
    OpenRouter,
    Groq,
    Mistral,
    XAI,
    Together,
    Fireworks,
    Cohere,
    Replicate,
    HuggingFace,
    AI21,
    Perplexity,
    Azure,
    Ollama,
    Cerebras,
    Novita,
    SambaNova,
    SiliconFlow,
    LeptonAI,
    DeepInfra,
    Nebius,
    Hyperbolic,
    Bedrock,
    Vertex,
    Voyage,
    Jina,
    Watsonx,
    Anyscale,
    FriendliAI,
    Baseten,
    OctoAI,
    Predibase,
    RunPod,
    PremAI,
    Spawning,
    Scaleway,
    OVHcloud,
}

impl ProviderId {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderId::OpenAI => "openai",
            ProviderId::Anthropic => "anthropic",
            ProviderId::Gemini => "gemini",
            ProviderId::DeepSeek => "deepseek",
            ProviderId::OpenRouter => "openrouter",
            ProviderId::Groq => "groq",
            ProviderId::Mistral => "mistral",
            ProviderId::XAI => "xai",
            ProviderId::Together => "together",
            ProviderId::Fireworks => "fireworks",
            ProviderId::Cohere => "cohere",
            ProviderId::Replicate => "replicate",
            ProviderId::HuggingFace => "huggingface",
            ProviderId::AI21 => "ai21",
            ProviderId::Perplexity => "perplexity",
            ProviderId::Azure => "azure",
            ProviderId::Ollama => "ollama",
            ProviderId::Cerebras => "cerebras",
            ProviderId::Novita => "novita",
            ProviderId::SambaNova => "sambanova",
            ProviderId::SiliconFlow => "siliconflow",
            ProviderId::LeptonAI => "lepton",
            ProviderId::DeepInfra => "deepinfra",
            ProviderId::Nebius => "nebius",
            ProviderId::Hyperbolic => "hyperbolic",
            ProviderId::Bedrock => "bedrock",
            ProviderId::Vertex => "vertex",
            ProviderId::Voyage => "voyage",
            ProviderId::Jina => "jina",
            ProviderId::Watsonx => "watsonx",
            ProviderId::Anyscale => "anyscale",
            ProviderId::FriendliAI => "friendli",
            ProviderId::Baseten => "baseten",
            ProviderId::OctoAI => "octoai",
            ProviderId::Predibase => "predibase",
            ProviderId::RunPod => "runpod",
            ProviderId::PremAI => "premai",
            ProviderId::Spawning => "spawning",
            ProviderId::Scaleway => "scaleway",
            ProviderId::OVHcloud => "ovhcloud",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderId::OpenAI => "OpenAI",
            ProviderId::Anthropic => "Anthropic (Claude)",
            ProviderId::Gemini => "Google Gemini",
            ProviderId::DeepSeek => "DeepSeek",
            ProviderId::OpenRouter => "OpenRouter",
            ProviderId::Groq => "Groq",
            ProviderId::Mistral => "Mistral AI",
            ProviderId::XAI => "xAI (Grok)",
            ProviderId::Together => "Together AI",
            ProviderId::Fireworks => "Fireworks AI",
            ProviderId::Cohere => "Cohere",
            ProviderId::Replicate => "Replicate",
            ProviderId::HuggingFace => "Hugging Face",
            ProviderId::AI21 => "AI21 Labs",
            ProviderId::Perplexity => "Perplexity",
            ProviderId::Azure => "Azure OpenAI",
            ProviderId::Ollama => "Ollama (local)",
            ProviderId::Cerebras => "Cerebras",
            ProviderId::Novita => "Novita AI",
            ProviderId::SambaNova => "SambaNova",
            ProviderId::SiliconFlow => "SiliconFlow",
            ProviderId::LeptonAI => "Lepton AI",
            ProviderId::DeepInfra => "DeepInfra",
            ProviderId::Nebius => "Nebius AI",
            ProviderId::Hyperbolic => "Hyperbolic",
            ProviderId::Bedrock => "AWS Bedrock",
            ProviderId::Vertex => "Google Vertex AI",
            ProviderId::Voyage => "Voyage AI",
            ProviderId::Jina => "Jina AI",
            ProviderId::Watsonx => "IBM Watsonx",
            ProviderId::Anyscale => "Anyscale",
            ProviderId::FriendliAI => "FriendliAI",
            ProviderId::Baseten => "Baseten",
            ProviderId::OctoAI => "OctoAI",
            ProviderId::Predibase => "Predibase",
            ProviderId::RunPod => "RunPod",
            ProviderId::PremAI => "PremAI",
            ProviderId::Spawning => "Spawning AI",
            ProviderId::Scaleway => "Scaleway",
            ProviderId::OVHcloud => "OVHcloud",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(ProviderId::OpenAI),
            "anthropic" | "claude" => Some(ProviderId::Anthropic),
            "gemini" | "google" => Some(ProviderId::Gemini),
            "deepseek" => Some(ProviderId::DeepSeek),
            "openrouter" => Some(ProviderId::OpenRouter),
            "groq" => Some(ProviderId::Groq),
            "mistral" => Some(ProviderId::Mistral),
            "xai" | "grok" => Some(ProviderId::XAI),
            "together" | "together-ai" => Some(ProviderId::Together),
            "fireworks" | "fireworks-ai" => Some(ProviderId::Fireworks),
            "cohere" => Some(ProviderId::Cohere),
            "replicate" => Some(ProviderId::Replicate),
            "huggingface" | "hugging-face" | "hf" => Some(ProviderId::HuggingFace),
            "ai21" => Some(ProviderId::AI21),
            "perplexity" => Some(ProviderId::Perplexity),
            "azure" | "azure-openai" => Some(ProviderId::Azure),
            "ollama" => Some(ProviderId::Ollama),
            "cerebras" => Some(ProviderId::Cerebras),
            "novita" => Some(ProviderId::Novita),
            "sambanova" => Some(ProviderId::SambaNova),
            "siliconflow" => Some(ProviderId::SiliconFlow),
            "lepton" | "leptonai" => Some(ProviderId::LeptonAI),
            "deepinfra" => Some(ProviderId::DeepInfra),
            "nebius" => Some(ProviderId::Nebius),
            "hyperbolic" => Some(ProviderId::Hyperbolic),
            "bedrock" | "aws" => Some(ProviderId::Bedrock),
            "vertex" | "google-vertex" => Some(ProviderId::Vertex),
            "voyage" | "voyageai" => Some(ProviderId::Voyage),
            "jina" => Some(ProviderId::Jina),
            "watsonx" | "ibm" => Some(ProviderId::Watsonx),
            "anyscale" => Some(ProviderId::Anyscale),
            "friendli" | "friendliai" => Some(ProviderId::FriendliAI),
            "baseten" => Some(ProviderId::Baseten),
            "octoai" => Some(ProviderId::OctoAI),
            "predibase" => Some(ProviderId::Predibase),
            "runpod" => Some(ProviderId::RunPod),
            "premai" => Some(ProviderId::PremAI),
            "spawning" => Some(ProviderId::Spawning),
            "scaleway" => Some(ProviderId::Scaleway),
            "ovhcloud" | "ovh" => Some(ProviderId::OVHcloud),
            _ => None,
        }
    }

    pub fn all() -> &'static [ProviderId] {
        &[
            ProviderId::OpenAI,
            ProviderId::Anthropic,
            ProviderId::Gemini,
            ProviderId::DeepSeek,
            ProviderId::OpenRouter,
            ProviderId::Groq,
            ProviderId::Mistral,
            ProviderId::XAI,
            ProviderId::Together,
            ProviderId::Fireworks,
            ProviderId::Cohere,
            ProviderId::Replicate,
            ProviderId::HuggingFace,
            ProviderId::AI21,
            ProviderId::Perplexity,
            ProviderId::Azure,
            ProviderId::Ollama,
            ProviderId::Cerebras,
            ProviderId::Novita,
            ProviderId::SambaNova,
            ProviderId::SiliconFlow,
            ProviderId::LeptonAI,
            ProviderId::DeepInfra,
            ProviderId::Nebius,
            ProviderId::Hyperbolic,
            ProviderId::Bedrock,
            ProviderId::Vertex,
            ProviderId::Voyage,
            ProviderId::Jina,
            ProviderId::Watsonx,
            ProviderId::Anyscale,
            ProviderId::FriendliAI,
            ProviderId::Baseten,
            ProviderId::OctoAI,
            ProviderId::Predibase,
            ProviderId::RunPod,
            ProviderId::PremAI,
            ProviderId::Spawning,
            ProviderId::Scaleway,
            ProviderId::OVHcloud,
        ]
    }

    /// Whether this provider is OpenAI wire-compatible (uses Bearer auth +
    /// /v1/chat/completions). Used to decide if we can reuse the OpenAI impl.
    pub fn is_openai_compatible(&self) -> bool {
        matches!(
            self,
            ProviderId::OpenAI
                | ProviderId::DeepSeek
                | ProviderId::OpenRouter
                | ProviderId::Groq
                | ProviderId::Mistral
                | ProviderId::XAI
                | ProviderId::Together
                | ProviderId::Fireworks
                | ProviderId::HuggingFace
                | ProviderId::AI21
                | ProviderId::Perplexity
                | ProviderId::Azure
                | ProviderId::Ollama
                | ProviderId::Cerebras
                | ProviderId::Novita
                | ProviderId::SambaNova
                | ProviderId::SiliconFlow
                | ProviderId::LeptonAI
                | ProviderId::DeepInfra
                | ProviderId::Nebius
                | ProviderId::Hyperbolic
                | ProviderId::Bedrock
                | ProviderId::Vertex
                | ProviderId::Voyage
                | ProviderId::Jina
                | ProviderId::Watsonx
                | ProviderId::Anyscale
                | ProviderId::FriendliAI
                | ProviderId::Baseten
                | ProviderId::OctoAI
                | ProviderId::Predibase
                | ProviderId::RunPod
                | ProviderId::PremAI
                | ProviderId::Spawning
                | ProviderId::Scaleway
                | ProviderId::OVHcloud
        )
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

