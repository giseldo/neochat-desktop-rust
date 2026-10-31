---
name: feature-release
description: Carregar sempre que qualquer mudança, melhoria ou funcionalidade for implementada/concluída neste projeto (neochat-desktop-rust). Executa obrigatoriamente o fluxo de release final: gerar pacote dist/NeoChat.exe, commit e push.
---

# Feature Release

Sempre depois de qualquer mudança de código neste projeto (nova funcionalidade, melhoria de UI, correção de bug ou refatoração), execute obrigatoriamente o fluxo completo de release abaixo, nesta ordem. Só prossiga para a etapa seguinte se a anterior tiver sucesso.

## 1. Build do binário e geração da pasta `dist`

Gere sempre o binário release e atualize a pasta `dist/` com o script de pacote do projeto:

```powershell
powershell.exe -ExecutionPolicy Bypass -File scripts/package.ps1
```

O script roda `cargo build --release` e copia o executável atualizado para `dist/NeoChat.exe` (junto com `dist/README.md`).

- Se o build falhar (ou se `NeoChat.exe` estiver em execução bloqueando a cópia), corrija ou avise o usuário. Não faça commit de código que não compila.
- `dist/` e `target/` estão no `.gitignore` — nunca os inclua no commit.

## 2. Commit

- Revise o que mudou com `git status` e `git diff` e adicione os arquivos relacionados à mudança.
- Mensagem no padrão conventional commits usado no repositório (`feat:`, `fix:`, `refactor:`, `chore:`).

## 3. Push

```powershell
git push origin master
```

- Confirme antes que o build do passo 1 passou e o commit do passo 2 foi criado.
- Se o push for rejeitado, rode `git pull --rebase origin master` e tente novamente; nunca use `--force`.

## 4. Resumo final

Informe ao usuário: o que foi commitado, o hash do commit, se o push foi concluído e confirme que `dist/NeoChat.exe` foi atualizado.
