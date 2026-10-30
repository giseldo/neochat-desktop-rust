use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Registro de cliente publico do Mistral Vibe CLI (issuer auth.mistral.ai).
/// Credencial dedicada ao CLI, reaproveitada aqui por nao haver registro
/// publico de clientes OAuth para apps de terceiros.
const ISSUER: &str = "https://auth.mistral.ai";
const CLIENT_ID: &str = "829da7e7-628f-4795-829a-8cca609c76d4";
const REDIRECT_PORT: u16 = 8766;
const REDIRECT_URI: &str = "http://127.0.0.1:8766/callback";
const SCOPE: &str = "openid offline_access email profile";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthTokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: i64,
}

/// Slot compartilhado com os tokens atuais; persiste via DPAPI ao alterar.
#[derive(Clone, Default)]
pub struct TokenStore(Arc<Mutex<Option<OauthTokens>>>);

impl TokenStore {
    pub fn from_disk() -> Self {
        Self(Arc::new(Mutex::new(crate::secrets::load_tokens())))
    }

    pub fn get(&self) -> Option<OauthTokens> {
        self.0.lock().ok()?.clone()
    }

    pub fn set(&self, tokens: OauthTokens) {
        if let Err(err) = crate::secrets::save_tokens(&tokens) {
            eprintln!("falha ao salvar tokens: {err:#}");
        }
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(tokens);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = None;
        }
        if let Err(err) = crate::secrets::clear_tokens() {
            eprintln!("falha ao remover tokens: {err:#}");
        }
    }
}

struct Pkce {
    verifier: String,
    challenge: String,
}

fn pkce() -> Pkce {
    let verifier = random_b64(48);
    let digest = Sha256::digest(verifier.as_bytes());
    Pkce {
        challenge: URL_SAFE_NO_PAD.encode(digest),
        verifier,
    }
}

fn random_b64(bytes: usize) -> String {
    use rand::RngCore;
    let mut buffer = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn into_tokens(raw: TokenResponse) -> OauthTokens {
    let expires_at = crate::storage::now() + raw.expires_in.unwrap_or(0).saturating_sub(60);
    OauthTokens {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_at,
    }
}

async fn token_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("falha ao criar cliente HTTP")
}

/// Fluxo completo: listener em 127.0.0.1:8766, navegador, troca do codigo.
pub async fn login() -> Result<OauthTokens> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", REDIRECT_PORT))
        .await
        .context(format!(
            "falha ao reservar a porta {REDIRECT_PORT}; feche outros processos que a usem"
        ))?;

    let pkce = pkce();
    let state = random_b64(16);
    let auth_url = format!(
        "{ISSUER}/oauth2/auth?response_type=code&client_id={CLIENT_ID}\
         &redirect_uri={redirect}&scope={scope}&state={state}\
         &code_challenge={challenge}&code_challenge_method=S256",
        redirect = urlencode(REDIRECT_URI),
        scope = urlencode(SCOPE),
        challenge = pkce.challenge,
    );

    open_browser(&auth_url)?;
    let code = wait_for_code(listener, &state).await?;
    exchange_code(&code, &pkce.verifier).await
}

/// Renova o access token com o refresh token guardado.
pub async fn refresh(tokens: &OauthTokens) -> Result<OauthTokens> {
    let Some(refresh_token) = tokens.refresh_token.as_deref() else {
        bail!("sessao sem refresh token; faca login novamente");
    };
    let raw: TokenResponse = token_client()
        .await?
        .post(format!("{ISSUER}/oauth2/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .context("falha ao renovar a sessao")?
        .error_for_status()
        .context("renovacao da sessao rejeitada; faca login novamente")?
        .json()
        .await
        .context("resposta invalida ao renovar a sessao")?;
    Ok(into_tokens(raw))
}

async fn exchange_code(code: &str, verifier: &str) -> Result<OauthTokens> {
    let raw: TokenResponse = token_client()
        .await?
        .post(format!("{ISSUER}/oauth2/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .context("falha ao trocar o codigo de autorizacao")?
        .error_for_status()
        .context("troca de codigo rejeitada pelo servidor de autenticacao")?
        .json()
        .await
        .context("resposta invalida na troca do codigo")?;
    Ok(into_tokens(raw))
}

async fn wait_for_code(
    listener: tokio::net::TcpListener,
    expected_state: &str,
) -> Result<String> {
    let deadline = tokio::time::Instant::now() + LOGIN_TIMEOUT;
    loop {
        let (stream, _) = tokio::time::timeout_at(deadline, listener.accept())
            .await
            .map_err(|_| anyhow::anyhow!("tempo esgotado aguardando o login no navegador"))?
            .context("falha ao aguardar o retorno do navegador")?;
        if let Some(code) = handle_callback(stream, expected_state).await? {
            return Ok(code);
        }
    }
}

/// Responde a conexao; `Ok(Some(code))` encerra o fluxo com sucesso.
async fn handle_callback(
    mut stream: tokio::net::TcpStream,
    expected_state: &str,
) -> Result<Option<String>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk)).await;
        match read {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => {
                buffer.extend_from_slice(&chunk[..n]);
                if buffer.windows(4).any(|w| w == b"\r\n\r\n") || buffer.len() > 16 * 1024 {
                    break;
                }
            }
            Ok(Err(err)) => return Err(err.into()),
        }
    }

    let request = String::from_utf8_lossy(&buffer);
    let Some(path) = request.split_whitespace().nth(1) else {
        respond(&mut stream, "400 Bad Request", "<p>Requisicao invalida.</p>").await?;
        return Ok(None);
    };

    let (path, query) = match path.split_once('?') {
        Some((path, query)) => (path, query),
        None => (path, ""),
    };

    if path != "/callback" {
        respond(&mut stream, "404 Not Found", "<p>Nao encontrado.</p>").await?;
        return Ok(None);
    }

    let params = parse_query(query);
    if params.get("state").map(String::as_str) != Some(expected_state) {
        respond(
            &mut stream,
            "400 Bad Request",
            "<p>Resposta com estado invalido; tente novamente.</p>",
        )
        .await?;
        return Ok(None);
    }

    if let Some(error) = params.get("error") {
        let detail = params.get("error_description").cloned().unwrap_or_default();
        respond(
            &mut stream,
            "200 OK",
            "<p>Falha no login; feche esta janela e tente novamente no NeoChat.</p>",
        )
        .await?;
        bail!("login cancelado ou negado: {error} {detail}");
    }

    let Some(code) = params.get("code").cloned() else {
        respond(
            &mut stream,
            "400 Bad Request",
            "<p>Codigo ausente; tente novamente.</p>",
        )
        .await?;
        bail!("retorno do navegador sem codigo de autorizacao");
    };

    respond(
        &mut stream,
        "200 OK",
        "<h2>Login concluido</h2><p>Pode fechar esta janela e voltar ao NeoChat.</p>",
    )
    .await?;
    Ok(Some(code))
}

async fn respond(stream: &mut tokio::net::TcpStream, status: &str, html: &str) -> Result<()> {
    let body = format!("<html><body>{html}</body></html>");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (percent_decode(key), percent_decode(value)))
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"),
                    16,
                ) {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn urlencode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .context("falha ao abrir o navegador")?;
    }
    #[cfg(not(windows))]
    {
        for opener in ["xdg-open", "open"] {
            if std::process::Command::new(opener)
                .arg(url)
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }
        bail!("nao foi possivel abrir o navegador");
    }
    #[cfg(windows)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_verifier() {
        let pkce = pkce();
        assert!(pkce.verifier.len() >= 43);
        let digest = Sha256::digest(pkce.verifier.as_bytes());
        assert_eq!(pkce.challenge, URL_SAFE_NO_PAD.encode(digest));
    }

    #[test]
    fn query_parsing_decodes_values() {
        let params = parse_query("code=abc%2B123&state=s%20t&error_description=x+y");
        assert_eq!(params.get("code").map(String::as_str), Some("abc+123"));
        assert_eq!(params.get("state").map(String::as_str), Some("s t"));
        assert_eq!(
            params.get("error_description").map(String::as_str),
            Some("x y")
        );
    }

    #[test]
    fn urlencoding_escapes_specials() {
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
    }

    #[test]
    fn tokens_keep_refresh_and_expiry() {
        let json = r#"{"access_token":"t","refresh_token":"r","expires_in":3600}"#;
        let raw: TokenResponse = serde_json::from_str(json).unwrap();
        let tokens = into_tokens(raw);
        assert_eq!(tokens.refresh_token.as_deref(), Some("r"));
        assert!(tokens.expires_at > crate::storage::now());
    }
}
