# Atalhos do Perene v2

> No macOS o modificador é **⌘ (Cmd)**; no Windows/Linux é **Ctrl+Shift** para o
> grupo de abas/painéis (evita colidir com as teclas do próprio terminal, como
> Ctrl+C/Ctrl+D). Também aparecem no diálogo de Configurações (⌘,).

## Abas e painéis

| macOS | Windows/Linux | Ação |
|---|---|---|
| ⌘T | Ctrl+Shift+T | Novo terminal (shell) na aba atual |
| ⌘W | Ctrl+Shift+W | Fechar o painel ativo (fecha a aba se for o último) |
| ⌘D | Ctrl+Shift+D | Dividir o painel à direita |
| ⌘⇧D | Ctrl+Alt+D | Dividir o painel abaixo |
| ⌘1 … ⌘9 | Ctrl+1 … Ctrl+9 | Ir para a aba N do workspace |
| ⌘, | Ctrl+, | Abrir/fechar Configurações |

## Terminal

| macOS | Windows/Linux | Ação |
|---|---|---|
| ⌘C | Ctrl+Shift+C | Copiar a seleção |
| ⌘V | Ctrl+Shift+V | Colar (texto **ou** imagem — imagem vira arquivo em `~/.perene2/paste/` e cola o caminho) |
| ⇧Enter | ⇧Enter | Nova linha sem enviar (Claude Code) |
| option+e, e | — | Dead keys / acentos (é, ã, ü…) — funcionam nativamente |

## Mouse / trackpad

- **Arrastar aba** na sidebar → reordena ou move para dentro/fora de uma pasta.
- **Duplo-clique** em workspace/pasta/aba → renomear.
- **Arrastar o divisor** entre painéis → ajusta a proporção (persistida).
- **Clicar num painel** → foca aquele terminal.

## Barra inferior

- Ícones de perfil (❯ shell, ✳ claude, ◆ codex, ◇ opencode) → novo terminal do perfil.
- Botões de split (⇥ direita, ⤓ abaixo) e presets de layout (▥ colunas, ▤ linhas, ▦ grade).
- ⚙ → Configurações (inclui o modo **YOLO**, que pula as permissões das CLIs).
