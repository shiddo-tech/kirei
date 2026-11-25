<div align="center">

<img src="assets/icon.png" alt="Ícone do Kirei" width="128" height="128">

# Kirei

**Capturas de tela bonitas e prontas para compartilhar, com um atalho.**

App para macOS feito em Rust com [GPUI](https://www.gpui.rs/), com ocultação automática de dados sensíveis (inclusive CPF e CNPJ).

![Versão](https://img.shields.io/badge/vers%C3%A3o-1.2.0-black)
![Plataforma](https://img.shields.io/badge/macOS-14%2B-black?logo=apple)
![Rust](https://img.shields.io/badge/Rust-2024-black?logo=rust)
![Licença](https://img.shields.io/badge/licen%C3%A7a-MIT-black)
![Idiomas](https://img.shields.io/badge/idiomas-EN%20%C2%B7%20PT--BR%20%C2%B7%20ES-black)

[Destaques](#destaques) · [Instalação](#instalação) · [Atalhos](#atalhos-globais) · [Editor](#editor) · [Privacidade](#privacidade) · [Arquitetura](#arquitetura) · [Desenvolvimento](#desenvolvimento) · [Distribuição](#distribuição) · [Contribuindo](#contribuindo) · [Licença](#licença)
</div>

---

## Sobre

O Kirei (綺麗, "bonito" em japonês) transforma uma captura de tela comum numa imagem pronta para post, documentação ou apresentação. Você captura, o app aplica fundo, moldura, sombra e cantos arredondados, e esconde e-mails, telefones, cartões, CPFs e chaves de API antes de você compartilhar.

Ele vive na barra de menus, responde a atalhos globais e funciona 100% no seu Mac. Não há conta, servidor nem envio de dados.

## Destaques

| | |
|---|---|
| **Captura completa** | Área, janela, tela inteira, captura com rolagem, gravação de tela (MP4/GIF) e OCR direto para a área de transferência. |
| **Composição automática** | Auto-equilíbrio, 27 fundos procedurais, molduras de janela e navegador, sombra em duas camadas e cantos contínuos (squircle). |
| **Dados sensíveis ocultos** | OCR com Vision e detecção de e-mail, telefone, cartão (Luhn), CPF, CNPJ, IP, chaves de API e JWT. |
| **Anotações** | Seta, retângulo, elipse, marca-texto, texto, contador numerado, pixelizar, desfocar e recorte, com guias de alinhamento. |
| **Fluxo rápido** | Miniatura flutuante, ⌘C imediato, fixar imagem na tela e histórico dos últimos 14 dias. |
| **Integrado ao macOS** | ScreenCaptureKit, Vision, vibração no trackpad e interface em três idiomas. Feito para chegar também ao Windows e ao Linux. |

## Requisitos

- macOS 14 (Sonoma) ou mais recente. A gravação de tela exige macOS 15.
- Para compilar: [Rust](https://rustup.rs) e as Command Line Tools do Xcode (`xcode-select --install`). O Xcode completo não é necessário.
- Permissão de **Gravação de Tela e Áudio do Sistema**

## Instalação

### Compilar a partir do código

Você não precisa de conta de desenvolvedor da Apple nem do Xcode completo.

```bash
git clone https://github.com/shiddo-tech/kirei.git
cd kirei
scripts/bundle-macos.sh --install
```

| Comando | O que faz |
|---|---|
| `scripts/bundle-macos.sh` | Compila em debug e monta `target/debug/Kirei.app` |
| `scripts/bundle-macos.sh --release` | Compila em release e monta `target/release/Kirei.app` |
| `scripts/bundle-macos.sh --install` | Compila em release, assina e copia para `/Applications` |

O primeiro build leva alguns minutos, porque o GPUI compila muita coisa. Os seguintes são incrementais.

### Primeira execução

1. Abra o Kirei. O onboarding apresenta o app e os atalhos.
2. Na primeira captura, autorize em **Ajustes do Sistema › Privacidade e Segurança › Gravação de Tela e Áudio do Sistema**.
3. Reabra o app.

### Assinatura local e permissões

O `bundle-macos.sh` assina o app com o certificado local **Kirei Local Signing**, guardado no keychain de login. Assim a identidade do app não muda entre builds e a permissão de Gravação de Tela continua valendo depois de recompilar.

Sem esse certificado, o build usa assinatura ad-hoc e o macOS pede a permissão de novo a cada build. Para compilar uma vez e usar, a assinatura ad-hoc basta. O certificado só faz diferença para quem recompila com frequência.

Para criar o certificado (uma vez só):

1. Abra o **Acesso às Chaves** e vá em **Acesso às Chaves › Assistente de Certificado › Criar um Certificado…**.
2. Em **Nome**, digite exatamente `Kirei Local Signing`.
3. Em **Tipo de Identidade**, escolha **Raiz Autoassinada**. Em **Tipo de Certificado**, escolha **Assinatura de Código**.
4. Clique em **Criar**. Na próxima execução, o `bundle-macos.sh` encontra o certificado e passa a usá-lo.

Para remover o certificado: **Acesso às Chaves › login › Meus Certificados › "Kirei Local Signing"**.

## Atalhos globais

Todos os atalhos podem ser trocados em **Ajustes**.

| Atalho | Ação |
|---|---|
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>1</kbd> | Capturar área. Arraste para selecionar; Espaço ou clique escolhe uma janela; Esc cancela. |
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>2</kbd> | Capturar janela |
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>3</kbd> | Capturar a tela sob o mouse |
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>4</kbd> | Capturar texto (OCR direto para a área de transferência) |
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>5</kbd> | Editar a imagem copiada |
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>6</kbd> | Gravar a tela (área ou janela). O mesmo atalho para a gravação. |
| <kbd>⌥</kbd><kbd>⇧</kbd><kbd>7</kbd> | Captura com rolagem: selecione a área e role para baixo devagar |

Em **Ajustes › Geral › Captura** há temporizador (3, 5 ou 10 s; clique na contagem para cancelar) e a opção de ocultar os ícones da mesa.

Na seleção de área, uma lupa mostra os pixels ao redor do cursor, com as coordenadas.

## Depois da captura

Por padrão, aparece uma **miniatura flutuante** no canto da tela. Dá para arrastá-la para outro app, copiar, salvar ou abrir o editor (botão ou clique duplo). Ela some depois de 7 s, mas fica enquanto o mouse estiver em cima. Em **Ajustes › Geral** dá para trocar esse comportamento por "abrir o editor" ou "copiar direto".

A captura tem flash e o som do Kirei: por padrão, uma voz suave dizendo "kirei" (`Resources/kirei-voice.wav`). Em **Ajustes › Geral** dá para trocar pelo som de obturador. Os outros sons são sintetizados no próprio app. A imagem desliza até a miniatura.

- **⌘C rápido**: enquanto a miniatura está na tela, ⌘C copia a imagem, mesmo com outro app em primeiro plano. Quando a miniatura some, o ⌘C volta a funcionar normalmente nos outros apps. No editor e numa imagem fixada, ⌘C também copia.
- **Fixar na tela**: pela miniatura, pelo editor (⌘P) ou pelo histórico. A imagem fica por cima de tudo. Arraste para mover, role ou faça pinça para redimensionar, clique duas vezes para editar e use o clique direito para opacidade e "deixar cliques passarem". Esc fecha. As imagens fixadas também fecham pela barra de menus.
- **Histórico**: barra de menus › Histórico de capturas (⌘Y no app). Guarda os originais dos últimos 14 dias (até 60) em `~/Library/Application Support/Kirei/History`. Dá para desligar em Ajustes.
- **Gravação** (macOS 15 ou mais novo): grava em MP4 (H.264, 60 fps, com cursor). Ao parar, a miniatura oferece salvar MP4, salvar GIF (12 fps, até 960 px) ou copiar. Durante a gravação, o ícone da barra de menus vira um botão de parar.

## Editor

### Composição

- **Auto-equilíbrio**: remove bordas uniformes e centraliza o conteúdo.
- **Fundo**: aba Arte com 27 fundos procedurais (mesh e degradês lineares e radiais, misturados em OKLab). O botão "Variar" mantém as cores e gera novas formas. Também há cores sólidas, cor personalizada, imagem, papel de parede ou transparente, com granulado opcional.
- **Moldura**: janela do macOS ou navegador, clara ou escura, e assinatura (@seunome) no canto.
- **Layout**: margem, inset (preenchido com a cor da borda), cantos arredondados, sombra, proporção (1:1, 4:3, 16:9, 9:16, 4:5…) e posição 3×3.
- **Moldura fina**: uma borda de meio ponto separa a captura do fundo (pode desligar em Layout). Os cantos são contínuos (squircle) e a sombra tem duas camadas, uma de contato e uma ambiente.

### Anotações

Seta, retângulo, elipse, marca-texto, texto, contador numerado, pixelizar, desfocar e recorte.

- **Seleção**: clique, ⇧+clique, arrastar um retângulo ou ⌘A.
- **Edição**: mover, redimensionar pelos cantos (o texto também), editar texto com duplo clique, duplicar (⌘D ou ⌥+arrastar), trazer para frente ou enviar para trás, apagar (⌫), mudar cor e espessura, desfazer e refazer.
- Com uma ferramenta de desenho ativa, arrastar pela borda de um objeto já o move. O clique direito abre o menu do objeto.
- **Contador**: o número se edita com duplo clique. Um número reinicia a sequência a partir dele. Um texto (uma letra, por exemplo) aparece sem ocupar um número. Apague o valor para voltar à numeração automática.
- **Guias de alinhamento**: ao mover anotações, elas encaixam no centro e nas bordas da captura e das outras anotações, com vibração no trackpad. Segure ⌘ para mover livre.

### Texto e dados sensíveis

- **Ocultar dados sensíveis**: OCR (Vision) e detecção de e-mail, telefone, cartão (Luhn), CPF, CNPJ, IP, chaves de API e JWT. Os trechos encontrados são pixelizados.
- **Copiar texto (OCR)**: extrai todo o texto da imagem.

### Exportação e estilos

- **Exportar**: copiar (⌘C), salvar na pasta padrão (⌘S), salvar como PNG, JPEG ou HEIC (⇧⌘S), arrastar para outros apps e compartilhar. Em Ajustes dá para exportar em @1x.
- **Estilos salvos**: cartões com miniatura no inspetor. Clique com o botão direito para atualizar ou apagar. O último estilo usado vira o padrão.

### Interface

Ferramentas parecidas dividem um botão (retângulo/elipse, pixelizar/desfocar). Clique de novo no botão ativo para alternar, ou use o clique direito. Cor e espessura ficam num único círculo, que só aparece com ferramenta de desenho ou anotação selecionada.

No inspetor, Moldura, Layout e Avançado (assinatura, inset, ângulo, granulado e moldura fina) são recolhíveis e lembram se estavam abertos. Os fundos mostram uma linha e um "Mostrar mais". Restaurar padrão e Atalhos ficam no menu ⋯.

### Teclas do editor

| Tecla | Ação |
|---|---|
| <kbd>V</kbd> <kbd>A</kbd> <kbd>R</kbd> <kbd>O</kbd> <kbd>H</kbd> <kbd>T</kbd> <kbd>N</kbd> <kbd>P</kbd> <kbd>B</kbd> <kbd>C</kbd> | Ferramentas |
| <kbd>⌫</kbd> | Apagar seleção |
| <kbd>←</kbd> <kbd>→</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | Mover 1 px (com <kbd>⇧</kbd>, 10 px) |
| <kbd>Esc</kbd> | Desmarcar |
| <kbd>⌘</kbd><kbd>C</kbd> / <kbd>⌘</kbd><kbd>S</kbd> / <kbd>⇧</kbd><kbd>⌘</kbd><kbd>S</kbd> | Copiar / Salvar / Salvar como |
| <kbd>⌘</kbd><kbd>D</kbd> | Duplicar |
| <kbd>⌘</kbd><kbd>P</kbd> | Fixar na tela |

## Privacidade

- **Tudo local.** O Kirei não faz nenhuma chamada de rede. OCR e detecção de dados sensíveis rodam no Mac, com o framework Vision da Apple.
- **Histórico sob seu controle.** Fica em `~/Library/Application Support/Kirei/History`, expira em 14 dias (máximo de 60 itens) e pode ser desligado em Ajustes.
- **Diagnósticos locais.** Relatórios de falha ficam em `~/Library/Logs/Kirei` e não são enviados para lugar nenhum.

> [!IMPORTANT]
> A ocultação automática é baseada em OCR e reconhecimento de padrões. Ela reduz muito o risco, mas não substitui uma revisão final da imagem antes de compartilhar.

## Arquitetura

O Kirei é um crate Rust com uma interface em [GPUI](https://www.gpui.rs/), o framework do editor Zed. O `scripts/bundle-macos.sh` monta o `.app` em volta do binário (Info.plist e ícone). Traduções e a voz "kirei" vão embutidas no binário. O app roda como agente da barra de menus (`LSUIElement`). O ícone no Dock só aparece enquanto há uma janela aberta.

A captura e as integrações com o sistema ficam em `src/capture` e `src/platform`. O resto do app não depende do macOS. Para levar o Kirei ao Windows ou ao Linux, crie os módulos dessas pastas para o novo sistema. A interface continua a mesma.

```
kirei/
├── Cargo.toml
├── build.rs                 # Linka as bibliotecas Swift que o crate screencapturekit usa
├── src/
│   ├── main.rs              # Ponto de entrada e modos de teste
│   ├── shell.rs             # Barra de menus, atalhos e menus do app
│   ├── flow.rs              # O que acontece depois de cada captura
│   ├── overlay.rs           # Seleção de área ou janela, com lupa
│   ├── scrolling.rs         # Captura com rolagem (interface)
│   ├── scroll.rs            # Costura das imagens da rolagem
│   ├── recorder.rs          # Gravação de tela e exportação em GIF
│   ├── editor/              # Editor: estado, canvas, inspetor e testes
│   ├── render/              # Composição (layout, fundos, sombra, texto, desfoque)
│   ├── model.rs             # Estilo, fundo e anotações
│   ├── analysis.rs, ocr.rs  # Auto-equilíbrio, OCR e dados sensíveis
│   ├── thumbnail.rs         # Miniatura flutuante
│   ├── pinned.rs            # Imagens fixadas na tela
│   ├── history*.rs          # Histórico e sua janela
│   ├── settings*.rs         # Preferências e janela de ajustes
│   ├── welcome.rs           # Onboarding
│   ├── feedback.rs, flash.rs, countdown.rs, hud.rs
│   ├── l10n.rs              # Função l() e escolha de idioma
│   ├── capture/             # Captura por sistema (macOS: ScreenCaptureKit)
│   ├── platform/            # AppKit, atalhos globais, Vision, vídeo e texto do macOS
│   └── ui/                  # Tema, ícones, controles e campo de texto
├── assets/                  # Ícone do app
├── Resources/               # Voz "kirei"
├── Localization/            # en, pt-BR e es (.lproj)
└── scripts/bundle-macos.sh  # Build, assinatura e instalação
```

## Desenvolvimento

```bash
cargo build                         # build de debug
cargo test                          # testes de modelos, traduções, OCR, exportação, histórico e editor
scripts/bundle-macos.sh             # monta o .app em target/debug/Kirei.app
```

> [!TIP]
> Para testar captura e permissões, use sempre o `.app`. Com `cargo run`, a permissão de Gravação de Tela fica com o terminal, não com o app.

### Modos de teste

O binário tem modos sem barra de menus nem atalhos globais:

```bash
kirei --render-test entrada.png saida.png [estilo.json]   # aplica um estilo a uma imagem
kirei --scroll-test imagem-alta.png saida.png             # simula uma captura com rolagem
kirei --record-test saida.mp4 [id-da-janela]              # grava 2 s de uma área ou janela
kirei --gif-test video.mp4                                # gera o pôster e o GIF de um vídeo
kirei --edit imagem.png                                   # abre só o editor
kirei --show settings|history|welcome|recording-controls  # abre só uma janela
```

Com `--edit` e `--show`, os dados ficam numa pasta temporária e o app não rouba o foco.

### Versionamento

- A versão fica em `Cargo.toml` (hoje, `1.2.0`) e vai para o Info.plist.
- O bundle identifier é `com.shiddo.kirei`.

### Idiomas

O app está em inglês, português (Brasil) e espanhol. Por padrão, segue o idioma do macOS. Dá para trocar em **Ajustes › Geral › Idioma** (é preciso reiniciar o app).

- No código, os textos usam `l("English text")`. A chave é o próprio texto em inglês, então uma tradução que faltar aparece em inglês.
- As traduções ficam em `Localization/<idioma>.lproj/Localizable.strings` e vão embutidas no binário.

Para incluir um idioma, crie a pasta `Localization/<idioma>.lproj` com as mesmas chaves e adicione o idioma em `src/l10n.rs`.

## Pendências

- DMG e notarização para distribuir fora do próprio Mac.
- Traduções e número de build no bundle (hoje o Info.plist só tem a versão).
- Gravação de tela no macOS 14 (o app usa `SCRecordingOutput`, que existe a partir do macOS 15).
- Windows e Linux.

## Contribuindo

Contribuições são bem-vindas: correções, traduções, ideias e relatos de bug. Leia o [CONTRIBUTING.md](CONTRIBUTING.md) antes de abrir um pull request.

Para relatar um bug, use os [templates de issue](https://github.com/shiddo-tech/kirei/issues/new/choose). Se o app travou, anexe os relatórios de `~/Library/Logs/Kirei`.

## Licença

Distribuído sob a [licença MIT](LICENSE).

---

<div align="center">

**Kirei** é um projeto open source da **Shiddo**.<br>
© 2026 Shiddo. Licença MIT.

</div>
