//! i18n — internationalization of API error messages.
//!
//! Languages: en, zh, es, fr, de, pt, ru, ja
//!
//! Set the response language via the Accept-Language header. Falls back to en.
//! For dashboard UI text, the React frontend handles its own i18n separately.

use std::collections::HashMap;
use once_cell::sync::Lazy;

pub enum Lang {
    En, Zh, Es, Fr, De, Pt, Ru, Ja,
}

impl Lang {
    pub fn from_header(header: Option<&str>) -> Self {
        let h = match header {
            Some(h) => h.to_lowercase(),
            None => return Lang::En,
        };
        // Parse the Accept-Language header (just look at the first lang code).
        let first = h.split(',').next().unwrap_or("").trim();
        let code = first.split('-').next().unwrap_or("").trim();
        match code {
            "zh" => Lang::Zh,
            "es" => Lang::Es,
            "fr" => Lang::Fr,
            "de" => Lang::De,
            "pt" => Lang::Pt,
            "ru" => Lang::Ru,
            "ja" => Lang::Ja,
            _ => Lang::En,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::En => "en", Lang::Zh => "zh", Lang::Es => "es", Lang::Fr => "fr",
            Lang::De => "de", Lang::Pt => "pt", Lang::Ru => "ru", Lang::Ja => "ja",
        }
    }
}

/// Translation keys for API error messages.
pub enum Msg {
    Unauthorized,
    Forbidden,
    NotFound,
    BadRequest,
    RateLimited,
    ProviderError,
    AllProvidersFailed,
    Internal,
    InvalidJwt,
    InvalidApiKey,
    UserNotFound,
    PasswordTooShort,
    CurrentPasswordIncorrect,
    UnknownProvider,
    InjectionDetected,
    SensitiveContent,
}

impl Msg {
    pub fn translate(&self, lang: &Lang) -> String {
        let table = MESSAGES.get(lang.code()).unwrap_or(&EN);
        let key = match self {
            Msg::Unauthorized => "unauthorized",
            Msg::Forbidden => "forbidden",
            Msg::NotFound => "not_found",
            Msg::BadRequest => "bad_request",
            Msg::RateLimited => "rate_limited",
            Msg::ProviderError => "provider_error",
            Msg::AllProvidersFailed => "all_providers_failed",
            Msg::Internal => "internal",
            Msg::InvalidJwt => "invalid_jwt",
            Msg::InvalidApiKey => "invalid_api_key",
            Msg::UserNotFound => "user_not_found",
            Msg::PasswordTooShort => "password_too_short",
            Msg::CurrentPasswordIncorrect => "current_password_incorrect",
            Msg::UnknownProvider => "unknown_provider",
            Msg::InjectionDetected => "injection_detected",
            Msg::SensitiveContent => "sensitive_content",
        };
        table.get(key).cloned().unwrap_or_else(|| EN.get(key).cloned().unwrap_or_default())
    }
}

type Catalog = Lazy<HashMap<&'static str, String>>;

static EN: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "Unauthorized"),
    ("forbidden", "Forbidden"),
    ("not_found", "Not found"),
    ("bad_request", "Bad request"),
    ("rate_limited", "Rate limited"),
    ("provider_error", "Provider error"),
    ("all_providers_failed", "All configured providers failed or are unavailable"),
    ("internal", "Internal server error"),
    ("invalid_jwt", "Invalid or expired token"),
    ("invalid_api_key", "Invalid API key"),
    ("user_not_found", "User not found"),
    ("password_too_short", "Password must be at least 8 characters"),
    ("current_password_incorrect", "Current password is incorrect"),
    ("unknown_provider", "Unknown provider"),
    ("injection_detected", "Prompt injection detected"),
    ("sensitive_content", "Sensitive content detected"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static ZH: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "未授权"),
    ("forbidden", "禁止访问"),
    ("not_found", "未找到"),
    ("bad_request", "请求错误"),
    ("rate_limited", "请求频率受限"),
    ("provider_error", "提供商错误"),
    ("all_providers_failed", "所有已配置的提供商均失败或不可用"),
    ("internal", "服务器内部错误"),
    ("invalid_jwt", "令牌无效或已过期"),
    ("invalid_api_key", "API 密钥无效"),
    ("user_not_found", "用户未找到"),
    ("password_too_short", "密码必须至少 8 个字符"),
    ("current_password_incorrect", "当前密码不正确"),
    ("unknown_provider", "未知的提供商"),
    ("injection_detected", "检测到提示注入"),
    ("sensitive_content", "检测到敏感内容"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static ES: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "No autorizado"),
    ("forbidden", "Prohibido"),
    ("not_found", "No encontrado"),
    ("bad_request", "Solicitud incorrecta"),
    ("rate_limited", "Límite de tasa"),
    ("provider_error", "Error del proveedor"),
    ("all_providers_failed", "Todos los proveedores configurados fallaron o no están disponibles"),
    ("internal", "Error interno del servidor"),
    ("invalid_jwt", "Token inválido o expirado"),
    ("invalid_api_key", "Clave API inválida"),
    ("user_not_found", "Usuario no encontrado"),
    ("password_too_short", "La contraseña debe tener al menos 8 caracteres"),
    ("current_password_incorrect", "La contraseña actual es incorrecta"),
    ("unknown_provider", "Proveedor desconocido"),
    ("injection_detected", "Inyección de prompt detectada"),
    ("sensitive_content", "Contenido sensible detectado"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static FR: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "Non autorisé"),
    ("forbidden", "Interdit"),
    ("not_found", "Introuvable"),
    ("bad_request", "Requête invalide"),
    ("rate_limited", "Limite de débit"),
    ("provider_error", "Erreur du fournisseur"),
    ("all_providers_failed", "Tous les fournisseurs configurés ont échoué ou sont indisponibles"),
    ("internal", "Erreur interne du serveur"),
    ("invalid_jwt", "Jeton invalide ou expiré"),
    ("invalid_api_key", "Clé API invalide"),
    ("user_not_found", "Utilisateur introuvable"),
    ("password_too_short", "Le mot de passe doit contenir au moins 8 caractères"),
    ("current_password_incorrect", "Le mot de passe actuel est incorrect"),
    ("unknown_provider", "Fournisseur inconnu"),
    ("injection_detected", "Injection de prompt détectée"),
    ("sensitive_content", "Contenu sensible détecté"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static DE: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "Nicht autorisiert"),
    ("forbidden", "Verboten"),
    ("not_found", "Nicht gefunden"),
    ("bad_request", "Ungültige Anfrage"),
    ("rate_limited", "Ratenlimit überschritten"),
    ("provider_error", "Anbieterfehler"),
    ("all_providers_failed", "Alle konfigurierten Anbieter sind fehlgeschlagen oder nicht verfügbar"),
    ("internal", "Interner Serverfehler"),
    ("invalid_jwt", "Ungültiges oder abgelaufenes Token"),
    ("invalid_api_key", "Ungültiger API-Schlüssel"),
    ("user_not_found", "Benutzer nicht gefunden"),
    ("password_too_short", "Passwort muss mindestens 8 Zeichen lang sein"),
    ("current_password_incorrect", "Aktuelles Passwort ist falsch"),
    ("unknown_provider", "Unbekannter Anbieter"),
    ("injection_detected", "Prompt-Injection erkannt"),
    ("sensitive_content", "Sensibler Inhalt erkannt"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static PT: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "Não autorizado"),
    ("forbidden", "Proibido"),
    ("not_found", "Não encontrado"),
    ("bad_request", "Requisição inválida"),
    ("rate_limited", "Limite de taxa"),
    ("provider_error", "Erro do provedor"),
    ("all_providers_failed", "Todos os provedores configurados falharam ou estão indisponíveis"),
    ("internal", "Erro interno do servidor"),
    ("invalid_jwt", "Token inválido ou expirado"),
    ("invalid_api_key", "Chave API inválida"),
    ("user_not_found", "Usuário não encontrado"),
    ("password_too_short", "A senha deve ter pelo menos 8 caracteres"),
    ("current_password_incorrect", "A senha atual está incorreta"),
    ("unknown_provider", "Provedor desconhecido"),
    ("injection_detected", "Injeção de prompt detectada"),
    ("sensitive_content", "Conteúdo sensível detectado"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static RU: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "Не авторизован"),
    ("forbidden", "Запрещено"),
    ("not_found", "Не найдено"),
    ("bad_request", "Неверный запрос"),
    ("rate_limited", "Превышен лимит"),
    ("provider_error", "Ошибка провайдера"),
    ("all_providers_failed", "Все настроенные провайдеры недоступны или завершились ошибкой"),
    ("internal", "Внутренняя ошибка сервера"),
    ("invalid_jwt", "Недействительный или истёкший токен"),
    ("invalid_api_key", "Недействительный API-ключ"),
    ("user_not_found", "Пользователь не найден"),
    ("password_too_short", "Пароль должен быть не менее 8 символов"),
    ("current_password_incorrect", "Текущий пароль неверен"),
    ("unknown_provider", "Неизвестный провайдер"),
    ("injection_detected", "Обнаружена инъекция промпта"),
    ("sensitive_content", "Обнаружен чувствительный контент"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static JA: Lazy<HashMap<&'static str, String>> = Lazy::new(|| [
    ("unauthorized", "認証されていません"),
    ("forbidden", "禁止されています"),
    ("not_found", "見つかりません"),
    ("bad_request", "無効なリクエスト"),
    ("rate_limited", "レート制限に達しました"),
    ("provider_error", "プロバイダーエラー"),
    ("all_providers_failed", "設定されたすべてのプロバイダーが失敗したか、利用できません"),
    ("internal", "内部サーバーエラー"),
    ("invalid_jwt", "無効または期限切れのトークン"),
    ("invalid_api_key", "無効なAPIキー"),
    ("user_not_found", "ユーザーが見つかりません"),
    ("password_too_short", "パスワードは8文字以上である必要があります"),
    ("current_password_incorrect", "現在のパスワードが正しくありません"),
    ("unknown_provider", "不明なプロバイダー"),
    ("injection_detected", "プロンプトインジェクションが検出されました"),
    ("sensitive_content", "機密コンテンツが検出されました"),
].iter().map(|(k, v)| (*k, v.to_string())).collect());

static MESSAGES: Lazy<HashMap<&'static str, &Lazy<HashMap<&'static str, String>>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, &Lazy<HashMap<&'static str, String>>> = HashMap::new();
    m.insert("en", &EN);
    m.insert("zh", &ZH);
    m.insert("es", &ES);
    m.insert("fr", &FR);
    m.insert("de", &DE);
    m.insert("pt", &PT);
    m.insert("ru", &RU);
    m.insert("ja", &JA);
    m
});
