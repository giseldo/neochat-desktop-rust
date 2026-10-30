use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::Duration;

/// Fluxo de login pelo navegador usado pelo Mistral Vibe CLI em producao:
/// processos efemeros de sign-in no console, sem client_id. O resultado
/// final e uma API key normal da Mistral.
const API_BASE: &str = "https://console.mistral.ai/api";
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct SignInProcess {
    process_id: String,
    sign_in_url: String,
    poll_url: String,
}

#[derive(Deserialize)]
struct PollResponse {
    status: String,
    #[serde(default)]
    exchange_token: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct ExchangeResponse {
    api_key: String,
}

struct Pkce {
    verifier: String,
    challenge: String,
}

fn pkce() -> Pkce {
    let verifier = random_b64(64);
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

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("falha ao criar cliente HTTP")
}

/// Fluxo completo: cria o processo de sign-in, abre o navegador,
/// aguarda a aprovacao e troca pelo API key.
pub async fn login() -> Result<String> {
    let pkce = pkce();
    let client = http_client()?;

    let process: SignInProcess = client
        .post(format!("{API_BASE}/vibe/sign-in"))
        .json(&serde_json::json!({
            "code_challenge": pkce.challenge,
            "code_challenge_method": "S256",
        }))
        .send()
        .await
        .context("falha ao iniciar o login no console da Mistral")?
        .error_for_status()
        .context("o console da Mistral recusou o inicio do login")?
        .json()
        .await
        .context("resposta invalida ao iniciar o login")?;

    open_browser(&process.sign_in_url)?;

    let exchange_token = poll_until_completed(&client, &process.poll_url).await?;
    let response = client
        .post(format!(
            "{API_BASE}/vibe/sign-in/{}/exchange",
            process.process_id
        ))
        .json(&serde_json::json!({
            "exchange_token": exchange_token,
            "code_verifier": pkce.verifier,
        }))
        .send()
        .await
        .context("falha ao finalizar o login")?
        .error_for_status()
        .context("a troca pelo API key foi rejeitada pelo console")?
        .json::<ExchangeResponse>()
        .await
        .context("resposta invalida ao finalizar o login")?;
    Ok(response.api_key)
}

async fn poll_until_completed(client: &reqwest::Client, poll_url: &str) -> Result<String> {
    let deadline = tokio::time::Instant::now() + LOGIN_TIMEOUT;
    loop {
        tokio::time::timeout_at(deadline, async {
            tokio::time::sleep(POLL_INTERVAL).await;
        })
        .await
        .map_err(|_| anyhow::anyhow!("tempo esgotado aguardando o login no navegador"))?;

        let response: PollResponse = client
            .get(poll_url)
            .send()
            .await
            .context("falha ao consultar o status do login")?
            .error_for_status()
            .context("status do login indisponivel")?
            .json()
            .await
            .context("resposta invalida ao consultar o status do login")?;

        match response.status.as_str() {
            "pending" => continue,
            "completed" => {
                let Some(token) = response.exchange_token else {
                    bail!("login concluido, mas o console nao devolveu o token de troca");
                };
                return Ok(token);
            }
            "expired" => bail!("o login no navegador expirou; tente novamente"),
            "denied" => bail!("o login foi negado no navegador"),
            other => bail!(
                "login retornou estado inesperado ({}){}",
                other,
                response
                    .message
                    .map(|m| format!(": {m}"))
                    .unwrap_or_default()
            ),
        }
    }
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
            if std::process::Command::new(opener).arg(url).spawn().is_ok() {
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
    fn verifiers_are_unique() {
        assert_ne!(random_b64(32), random_b64(32));
    }

    #[test]
    fn sign_in_process_parses_console_payload() {
        let json = r#"{"process_id":"p1","sign_in_url":"https://console.mistral.ai/x","poll_url":"https://console.mistral.ai/api/vibe/sign-in/p1"}"#;
        let process: SignInProcess = serde_json::from_str(json).unwrap();
        assert_eq!(process.process_id, "p1");
    }
}
