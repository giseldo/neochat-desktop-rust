use anyhow::{Context, Result};
use neochat::mistral::{Message, MistralClient};
use neochat::storage::Store;
use tokio::io::{AsyncBufReadExt, BufReader};

const SYSTEM_PROMPT: &str =
    "Você é o NeoChat, um assistente prestativo, claro e conciso. Responda no idioma do usuário.";

fn print_help() {
    println!("Comandos:");
    println!("  /ajuda   mostra esta ajuda");
    println!("  /limpar  apaga o histórico da conversa");
    println!("  /modelo  mostra o modelo em uso");
    println!("  /sair    encerra o programa");
}

#[tokio::main]
async fn main() -> Result<()> {
    let store = Store::load();

    let api_key = std::env::var("MISTRAL_API_KEY")
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .or_else(neochat::secrets::load_api_key)
        .context(
            "defina a variável MISTRAL_API_KEY ou faça login no app gráfico (Configurações > Conta Mistral)",
        )?;

    let model = std::env::var("MISTRAL_MODEL")
        .ok()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| store.config.model.clone());

    let client = MistralClient::new(api_key, model);
    let mut history = vec![Message::new("system", SYSTEM_PROMPT)];

    println!("NeoChat — modelo: {}", client.model);
    println!("Digite /ajuda para ver os comandos.\n");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    loop {
        print!("Você> ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let Some(line) = lines.next_line().await? else {
            break;
        };
        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            "/sair" => break,
            "/ajuda" => {
                print_help();
                continue;
            }
            "/limpar" => {
                history.truncate(1);
                println!("Histórico apagado.");
                continue;
            }
            "/modelo" => {
                println!("Modelo: {}", client.model);
                continue;
            }
            _ => {}
        }

        history.push(Message::new("user", input));
        print!("Bot> ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let result = client
            .chat_stream(&history, |token| {
                print!("{token}");
                std::io::Write::flush(&mut std::io::stdout()).ok();
            })
            .await;
        println!();
        match result {
            Ok(reply) => history.push(Message::new("assistant", reply)),
            Err(err) => {
                eprintln!("\nErro: {err:#}");
                history.pop();
            }
        }
    }

    println!("Até logo!");
    Ok(())
}
