//! Configuration — loaded from env vars + .env file.
//!
//! Mirrors the OmniRoute env contract (see .env.example). All values have
//! sensible defaults except JWT_SECRET and API_KEY_SECRET, which MUST be set.

use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub jwt_secret: String,
    pub api_key_secret: String,
    pub initial_password: String,
    pub port: u16,
    pub host: String,
    pub log_level: String,
    pub data_dir: PathBuf,
    pub db_url: String,
    pub dashboard_dist: Option<PathBuf>,
    pub failover_order: Vec<String>,
    pub max_retries: u32,
    pub request_timeout_secs: u64,
    pub provider_keys: ProviderKeys,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderKeys {
    pub openai: Option<String>,
    pub anthropic: Option<String>,
    pub gemini: Option<String>,
    pub deepseek: Option<String>,
    pub openrouter: Option<String>,
    pub groq: Option<String>,
    pub mistral: Option<String>,
    pub xai: Option<String>,
    pub together: Option<String>,
    pub fireworks: Option<String>,
    pub cohere: Option<String>,
    pub replicate: Option<String>,
    pub huggingface: Option<String>,
    pub ai21: Option<String>,
    pub perplexity: Option<String>,
    pub azure: Option<String>,
    pub ollama: Option<String>,    // Ollama doesn't need a key — set to "ollama" to enable
    pub cerebras: Option<String>,
    pub novita: Option<String>,
    pub sambanova: Option<String>,
    pub siliconflow: Option<String>,
    pub lepton: Option<String>,
    pub deepinfra: Option<String>,
    pub nebius: Option<String>,
    pub hyperbolic: Option<String>,
    pub bedrock: Option<String>,
    pub vertex: Option<String>,
    pub voyage: Option<String>,
    pub jina: Option<String>,
    pub watsonx: Option<String>,
    pub anyscale: Option<String>,
    pub friendli: Option<String>,
    pub baseten: Option<String>,
    pub octoai: Option<String>,
    pub predibase: Option<String>,
    pub runpod: Option<String>,
    pub premai: Option<String>,
    pub spawning: Option<String>,
    pub scaleway: Option<String>,
    pub ovhcloud: Option<String>,
}

impl Config {
    /// Load config from env vars (with .env file loaded first via dotenvy).
    pub fn from_env() -> anyhow::Result<Self> {
        // Load .env if present (no error if missing)
        let _ = dotenvy::dotenv();

        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| anyhow::anyhow!("JWT_SECRET must be set (see .env.example)"))?;
        let api_key_secret = env::var("API_KEY_SECRET")
            .map_err(|_| anyhow::anyhow!("API_KEY_SECRET must be set (see .env.example)"))?;

        if jwt_secret.len() < 16 {
            anyhow::bail!("JWT_SECRET is too short (minimum 16 chars)");
        }
        if api_key_secret.len() < 32 {
            anyhow::bail!("API_KEY_SECRET must be at least 32 hex chars (use `openssl rand -hex 32`)");
        }

        let port = env::var("PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(20128);
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into());
        let initial_password = env::var("INITIAL_PASSWORD").unwrap_or_else(|_| "CHANGEME".into());

        let data_dir = resolve_data_dir();
        std::fs::create_dir_all(&data_dir).map_err(|e| {
            anyhow::anyhow!("failed to create data dir {}: {}", data_dir.display(), e)
        })?;

        let db_path = data_dir.join("omniroute.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let dashboard_dist = env::var("DASHBOARD_DIST")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists());

        let failover_order = env::var("FAILOVER_ORDER")
            .ok()
            .map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect())
            .unwrap_or_else(|| {
                vec![
                    "openai".into(),
                    "anthropic".into(),
                    "gemini".into(),
                    "deepseek".into(),
                    "openrouter".into(),
                    "groq".into(),
                    "mistral".into(),
                    "xai".into(),
                    "together".into(),
                    "fireworks".into(),
                    "cohere".into(),
                    "perplexity".into(),
                    "ai21".into(),
                    "huggingface".into(),
                    "replicate".into(),
                    "azure".into(),
                    "ollama".into(),
                    "cerebras".into(),
                    "novita".into(),
                    "sambanova".into(),
                    "siliconflow".into(),
                    "lepton".into(),
                    "deepinfra".into(),
                    "nebius".into(),
                    "hyperbolic".into(),
                    "bedrock".into(),
                    "vertex".into(),
                    "voyage".into(),
                    "jina".into(),
                    "watsonx".into(),
                    "anyscale".into(),
                    "friendli".into(),
                    "baseten".into(),
                    "octoai".into(),
                    "predibase".into(),
                    "runpod".into(),
                    "premai".into(),
                    "spawning".into(),
                    "scaleway".into(),
                    "ovhcloud".into(),
                ]
            });

        let max_retries = env::var("MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        let request_timeout_secs = env::var("REQUEST_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120);

        let provider_keys = ProviderKeys {
            openai: env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty()),
            anthropic: env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty()),
            gemini: env::var("GEMINI_API_KEY").ok().filter(|s| !s.is_empty()),
            deepseek: env::var("DEEPSEEK_API_KEY").ok().filter(|s| !s.is_empty()),
            openrouter: env::var("OPENROUTER_API_KEY").ok().filter(|s| !s.is_empty()),
            groq: env::var("GROQ_API_KEY").ok().filter(|s| !s.is_empty()),
            mistral: env::var("MISTRAL_API_KEY").ok().filter(|s| !s.is_empty()),
            xai: env::var("XAI_API_KEY").ok().or_else(|| env::var("GROK_API_KEY").ok()).filter(|s| !s.is_empty()),
            together: env::var("TOGETHER_API_KEY").ok().filter(|s| !s.is_empty()),
            fireworks: env::var("FIREWORKS_API_KEY").ok().filter(|s| !s.is_empty()),
            cohere: env::var("COHERE_API_KEY").ok().filter(|s| !s.is_empty()),
            replicate: env::var("REPLICATE_API_TOKEN").ok().filter(|s| !s.is_empty()),
            huggingface: env::var("HUGGINGFACE_API_KEY").ok().or_else(|| env::var("HF_API_KEY").ok()).filter(|s| !s.is_empty()),
            ai21: env::var("AI21_API_KEY").ok().filter(|s| !s.is_empty()),
            perplexity: env::var("PERPLEXITY_API_KEY").ok().filter(|s| !s.is_empty()),
            azure: env::var("AZURE_OPENAI_API_KEY").ok().filter(|s| !s.is_empty()),
            ollama: env::var("OLLAMA_BASE_URL").ok().or_else(|| Some("http://localhost:11434".into())),
            cerebras: env::var("CEREBRAS_API_KEY").ok().filter(|s| !s.is_empty()),
            novita: env::var("NOVITA_API_KEY").ok().filter(|s| !s.is_empty()),
            sambanova: env::var("SAMBANOVA_API_KEY").ok().filter(|s| !s.is_empty()),
            siliconflow: env::var("SILICONFLOW_API_KEY").ok().filter(|s| !s.is_empty()),
            lepton: env::var("LEPTON_API_KEY").ok().filter(|s| !s.is_empty()),
            deepinfra: env::var("DEEPINFRA_API_KEY").ok().filter(|s| !s.is_empty()),
            nebius: env::var("NEBIUS_API_KEY").ok().filter(|s| !s.is_empty()),
            hyperbolic: env::var("HYPERBOLIC_API_KEY").ok().filter(|s| !s.is_empty()),
            bedrock: env::var("AWS_BEDROCK_API_KEY").ok().filter(|s| !s.is_empty()),
            vertex: env::var("VERTEX_ACCESS_TOKEN").ok().filter(|s| !s.is_empty()),
            voyage: env::var("VOYAGE_API_KEY").ok().filter(|s| !s.is_empty()),
            jina: env::var("JINA_API_KEY").ok().filter(|s| !s.is_empty()),
            watsonx: env::var("WATSONX_API_KEY").ok().filter(|s| !s.is_empty()),
            anyscale: env::var("ANYSCALE_API_KEY").ok().filter(|s| !s.is_empty()),
            friendli: env::var("FRIENDLI_API_KEY").ok().filter(|s| !s.is_empty()),
            baseten: env::var("BASETEN_API_KEY").ok().filter(|s| !s.is_empty()),
            octoai: env::var("OCTOAI_API_KEY").ok().filter(|s| !s.is_empty()),
            predibase: env::var("PREDIBASE_API_KEY").ok().filter(|s| !s.is_empty()),
            runpod: env::var("RUNPOD_API_KEY").ok().filter(|s| !s.is_empty()),
            premai: env::var("PREMAI_API_KEY").ok().filter(|s| !s.is_empty()),
            spawning: env::var("SPAWNING_API_KEY").ok().filter(|s| !s.is_empty()),
            scaleway: env::var("SCALEWAY_API_KEY").ok().filter(|s| !s.is_empty()),
            ovhcloud: env::var("OVHCLOUD_API_KEY").ok().filter(|s| !s.is_empty()),
        };

        Ok(Self {
            jwt_secret,
            api_key_secret,
            initial_password,
            port,
            host,
            log_level,
            data_dir,
            db_url,
            dashboard_dist,
            failover_order,
            max_retries,
            request_timeout_secs,
            provider_keys,
        })
    }

    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Resolve the data directory. Mirrors the Node.js OmniRoute's `resolveDataDir()`:
///   1. DATA_DIR env var (highest priority)
///   2. Windows: %APPDATA%/omniroute
///   3. Unix: $XDG_CONFIG_HOME/omniroute or ~/.omniroute
fn resolve_data_dir() -> PathBuf {
    if let Ok(d) = env::var("DATA_DIR") {
        let trimmed = d.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            return PathBuf::from(appdata).join("omniroute");
        }
        return dirs_home().map(|h| h.join("AppData").join("Roaming").join("omniroute"))
            .unwrap_or_else(|| PathBuf::from("omniroute"));
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join("omniroute");
            }
        }
        return dirs_home().map(|h| h.join(".omniroute"))
            .unwrap_or_else(|| PathBuf::from("omniroute"));
    }
}

#[allow(dead_code)]
fn dirs_home() -> Option<PathBuf> {
    env::var("HOME").ok().map(PathBuf::from)
}
