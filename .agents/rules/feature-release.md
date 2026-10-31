---
trigger: always_on
---

# Fluxo Obrigatório Pós-Mudança (Build `dist`, Commit e Push)

Sempre após implementar qualquer alteração de código neste repositório (`neochat-desktop-rust`):
1. Execute `powershell.exe -ExecutionPolicy Bypass -File scripts/package.ps1` para compilar em modo `--release` e atualizar `dist/NeoChat.exe`.
2. Faça `git add` e `git commit` das alterações com uma mensagem no padrão Conventional Commits (`feat:`, `fix:`, `refactor:`, `chore:`).
3. Faça `git push origin master`.
4. Informe no resumo final o hash do commit, o status do push e a atualização de `dist/NeoChat.exe`.
