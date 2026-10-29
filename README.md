# NeoChat

Cliente desktop em Rust para conversar com modelos da [Mistral](https://mistral.ai) via API, com streaming, Markdown, histórico de conversas e interface nativa em egui/eframe para Windows.

## Funcionalidades

- Streaming de respostas com botão **Parar** (ou Esc).
- Markdown na resposta (código, listas, tabelas) com botões **Copiar** e **Regenerar**.
- Conversas múltiplas na barra lateral: criar, renomear (menu de contexto) e excluir.
- Tema claro, escuro ou do sistema, com barra de título nativa acompanhando.
- Chave de API salva com DPAPI (criptografia por usuário do Windows).
- Atalhos: Enter envia, Shift+Enter quebra linha, Esc interrompe, Ctrl+N nova conversa, Ctrl+, configurações.
- Versão de terminal (`neochat-cli`).

## Requisitos

- Windows 10/11 x64.
- Rust estável (testado com 1.99.1) com toolchain MSVC.
- Visual Studio Build Tools com "Desktop development with C++" (linker e Windows SDK, incluindo `rc.exe` para o ícone).
- GPU com driver atualizado (renderização via wgpu/D3D12).

## Como rodar

```powershell
cargo run --release
```

Na primeira execução, abra **Configurações** no canto superior direito e cole a chave criada em <https://console.mistral.ai/api-keys>. Também é possível usar a variável de ambiente:

```powershell
$env:MISTRAL_API_KEY = "sua-chave"
cargo run --release
```

## Build do executável

```powershell
.\scripts\package.ps1
```

O script roda `cargo build --release` e gera `dist\NeoChat.exe` junto com o README. Basta copiar a pasta `dist` para outro PC.

## Versão de terminal

```powershell
$env:MISTRAL_API_KEY = "sua-chave"
cargo run --release --bin neochat-cli
```

Comandos: `/ajuda`, `/limpar`, `/modelo`, `/sair`. Se a variável não estiver definida, a CLI usa a chave salva pelo app.

## Dados e privacidade

Tudo fica em `%APPDATA%\NeoChat`:

| Arquivo | Conteúdo |
| --- | --- |
| `config.json` | modelo, tema e tamanho da janela |
| `conversations.json` | conversas e mensagens |
| `secret.dat` | chave de API cifrada com DPAPI (usuário atual) |

As mensagens só saem do computador quando você envia algo para a API da Mistral.

## Estrutura

```
src/
  main.rs             inicialização da janela
  bin/neochat-cli.rs  versão de terminal
  app/                interface (sidebar, chat, composer, settings)
  mistral.rs          cliente HTTP e streaming da API
  secrets.rs          DPAPI para a chave
  storage.rs          persistência em JSON
  theme.rs            fontes, cores e temas
assets/               ícone e manifesto
build.rs              compila o ícone (.rc) no executável
scripts/              geração de ícone e empacotamento
```

## Testes e verificação

```powershell
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```
