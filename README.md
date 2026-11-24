<div align="center">

<img src="docs/icon.png" alt="Ícone do Kirei" width="128" height="128">

# Kirei

**Capturas de tela bonitas e prontas para compartilhar, com um atalho.**

App nativo para macOS, feito em Swift e SwiftUI, com ocultação automática de dados sensíveis (inclusive CPF e CNPJ).

![Versão](https://img.shields.io/badge/vers%C3%A3o-1.2.0-black)
![Plataforma](https://img.shields.io/badge/macOS-14%2B-black?logo=apple)
![Swift](https://img.shields.io/badge/Swift-5.9-black?logo=swift&logoColor=F05138)
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
| **Nativo de verdade** | ScreenCaptureKit, Liquid Glass no macOS 26, vibração no trackpad e interface em três idiomas. |

## Requisitos

- macOS 14 (Sonoma) ou mais recente
- Command Line Tools do Xcode (`xcode-select --install`). O Xcode completo não é necessário.
- Permissão de **Gravação de Tela e Áudio do Sistema**

## Instalação

### Baixar pronto

Baixe o `Kirei-<versão>.dmg` mais recente em [Releases](https://github.com/shiddo-tech/kirei/releases), abra o arquivo e arraste o Kirei para **Aplicativos**.

> [!IMPORTANT]
> Se o DMG não estiver notarizado pela Apple, o macOS bloqueia a primeira abertura. Para liberar, clique com o botão direito no app, escolha **Abrir** e confirme. Outra opção é ir em **Ajustes do Sistema › Privacidade e Segurança** e clicar em **Abrir Mesmo Assim**.

### Compilar a partir do código

Você não precisa de conta de desenvolvedor da Apple nem do Xcode completo.

```bash
git clone https://github.com/shiddo-tech/kirei.git
cd kirei
scripts/build.sh --install
```

| Comando | O que faz |
|---|---|
| `scripts/build.sh` | Compila em modo release e monta `build/Kirei.app` |
| `scripts/build.sh --install` | Compila, gera o ícone, assina e copia para `/Applications` |
| `scripts/build.sh --dmg` | Gera `build/Kirei-<versão>.dmg`, com fundo e layout próprios |

> [!NOTE]
> Na geração do DMG, o Finder pede permissão de Automação na primeira vez. Sem ela, o DMG é gerado do mesmo jeito, só que sem o layout da janela.

### Primeira execução

1. Abra o Kirei. O onboarding apresenta o app e os atalhos.
2. Na primeira captura, autorize em **Ajustes do Sistema › Privacidade e Segurança › Gravação de Tela e Áudio do Sistema**.
3. Reabra o app.

### Assinatura local e permissões

O `build.sh` assina o app com o certificado local **Kirei Local Signing**, guardado no keychain de login. Assim a identidade do app não muda entre builds e a permissão de Gravação de Tela continua valendo depois de recompilar.

Sem esse certificado, o build usa assinatura ad-hoc e o macOS pede a permissão de novo a cada build. Para compilar uma vez e usar, a assinatura ad-hoc basta. O certificado só faz diferença para quem recompila com frequência.

Para criar o certificado (uma vez só):

1. Abra o **Acesso às Chaves** e vá em **Acesso às Chaves › Assistente de Certificado › Criar um Certificado…**.
2. Em **Nome**, digite exatamente `Kirei Local Signing`.
3. Em **Tipo de Identidade**, escolha **Raiz Autoassinada**. Em **Tipo de Certificado**, escolha **Assinatura de Código**.
4. Clique em **Criar**. Na próxima execução, o `build.sh` encontra o certificado e passa a usá-lo.

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

A captura tem flash e o som do Kirei: por padrão, uma voz suave dizendo "kirei" (`Resources/kirei-voice.wav`, gerada por `scripts/make-voice.swift`). Em **Ajustes › Geral** dá para trocar pelo som de obturador. Os outros sons são sintetizados no próprio app. A imagem desliza até a miniatura.

- **⌘C rápido**: enquanto a miniatura está na tela, ⌘C copia a imagem, mesmo com outro app em primeiro plano. Quando a miniatura some, o ⌘C volta a funcionar normalmente nos outros apps. No editor e numa imagem fixada, ⌘C também copia.
- **Fixar na tela**: pela miniatura, pelo editor (⌘P) ou pelo histórico. A imagem fica por cima de tudo. Arraste para mover, role ou faça pinça para redimensionar, clique duas vezes para editar e use o clique direito para opacidade e "deixar cliques passarem". Esc fecha. As imagens fixadas também fecham pela barra de menus.
- **Histórico**: barra de menus › Histórico de capturas (⌘Y no app). Guarda os originais dos últimos 14 dias (até 60) em `~/Library/Application Support/Kirei/History`. Dá para desligar em Ajustes.
- **Gravação**: grava em MP4 (H.264, 60 fps, com cursor). Ao parar, a miniatura oferece salvar MP4, salvar GIF (12 fps, até 960 px) ou copiar. Durante a gravação, o ícone da barra de menus vira um botão de parar.

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

- **Tudo local.** O Kirei não faz nenhuma chamada de rede. OCR e detecção de dados sensíveis rodam no Mac, com o framework Vision.
- **Histórico sob seu controle.** Fica em `~/Library/Application Support/Kirei/History`, expira em 14 dias (máximo de 60 itens) e pode ser desligado em Ajustes.
- **Diagnósticos locais.** Relatórios do MetricKit ficam em `~/Library/Logs/Kirei` e não são enviados para lugar nenhum.

> [!IMPORTANT]
> A ocultação automática é baseada em OCR e reconhecimento de padrões. Ela reduz muito o risco, mas não substitui uma revisão final da imagem antes de compartilhar.

## Arquitetura

O Kirei é um pacote Swift (`Package.swift`) com um único executável. O `build.sh` monta o `.app` em volta dele (Info.plist, ícone, sons e traduções). O app roda como agente da barra de menus (`LSUIElement`), sem ícone no Dock.

**Frameworks:** SwiftUI, AppKit, ScreenCaptureKit, AVFoundation, Vision, Carbon (atalhos globais), ServiceManagement (abrir ao iniciar) e MetricKit.

```
kirei/
├── Package.swift            # Manifesto SwiftPM (macOS 14+)
├── VERSION                  # Versão de marketing (CFBundleShortVersionString)
├── Sources/Kirei/           # Código-fonte do app
├── Resources/               # Sons empacotados (kirei-voice.wav)
├── Localization/            # en, pt-BR e es (.lproj)
└── scripts/
    ├── build.sh             # Build, assinatura, instalação, DMG e notarização
    ├── make-icon.swift      # Gera o ícone 1024 px
    ├── make-dmg-background.swift
    └── make-voice.swift     # Gera a voz "kirei"
```

### Módulos

| Arquivo | Responsabilidade |
|---|---|
| `main.swift` / `AppDelegate.swift` | Ponto de entrada, barra de menus, atalhos e autotestes |
| `Capture.swift` | Captura com ScreenCaptureKit (usando a permissão do próprio Kirei), exportação e HUD |
| `SelectionOverlay.swift` | Sobreposição de seleção de área ou janela, com lupa |
| `ScrollingCapture.swift` | Captura com rolagem (costura por hash das linhas) |
| `Recorder.swift` | Gravação de tela (ScreenCaptureKit + AVAssetWriter) e exportação em GIF |
| `Renderer.swift` | Composição (layout, fundo, sombra) e desenho das anotações |
| `Art.swift` | Fundos procedurais (campos calculados por pixel, com cache) |
| `ImageAnalysis.swift` | Auto-equilíbrio, OCR e detecção de dados sensíveis |
| `EditorModel.swift` / `EditorView.swift` / `EditorWindowController.swift` | Estado, interface e janela do editor |
| `CanvasView.swift` | Pré-visualização interativa e manipulação das anotações |
| `Annotation.swift` / `Style.swift` | Modelos de anotação e de estilo |
| `FloatingPreview.swift` | Miniatura flutuante depois da captura |
| `PinnedImage.swift` | Imagens fixadas na tela |
| `History.swift` | Histórico de capturas e sua janela |
| `CaptureFeedback.swift` | Sons sintetizados, flash, animação até a miniatura, temporizador e vibração |
| `Settings.swift` / `SettingsView.swift` | Preferências (janela com abas), presets e atalhos globais (Carbon) |
| `Onboarding.swift` | Boas-vindas, permissão de Gravação de Tela e atalhos |
| `Chrome.swift` | Vibrancy, Liquid Glass (macOS 26), área de arrastar da janela e miniaturas de estilos |
| `Localization.swift` | Função `L()` e escolha de idioma |
| `Diagnostics.swift` | Relatórios do MetricKit em `~/Library/Logs/Kirei` |
| `Logo.swift` | Logotipo desenhado em código |

## Desenvolvimento

```bash
swift build                 # build de debug em .build/debug/Kirei
swift build -c release      # build de release
scripts/build.sh            # monta o .app completo em build/Kirei.app
```

> [!TIP]
> Para testar captura e permissões, use sempre o `.app` gerado pelo `build.sh`. O binário solto em `.build/` não tem Info.plist, ícone nem traduções.

### Autotestes

O binário aceita dois modos de teste, sem interface:

```bash
# Renderizador: aplica um estilo a uma imagem e salva o resultado
.build/debug/Kirei --render-test entrada.png saida.png [estilo.json]

# Captura com rolagem: simula uma rolagem com cabeçalho e rodapé fixos
.build/debug/Kirei --scroll-test imagem-alta.png saida.png
```

### Versionamento

- A versão de marketing fica no arquivo `VERSION` (hoje, `1.2.0`).
- O número do build é gerado a cada compilação, a partir da data e hora (`AAAAMMDDhhmm`).
- O bundle identifier é `com.shiddo.kirei`.

### Idiomas

O app está em inglês, português (Brasil) e espanhol. Por padrão, segue o idioma do macOS. Dá para trocar em **Ajustes › Sistema › Idioma** (é preciso reiniciar o app).

- No código, os textos usam `L("English text")`. A chave é o próprio texto em inglês, então uma tradução que faltar aparece em inglês.
- As traduções ficam em `Localization/<idioma>.lproj/Localizable.strings`. O `scripts/build.sh` copia essas pastas para dentro do app (o `pt-BR` também é copiado como `pt`, para cobrir o português fora do Brasil).

Para incluir um idioma:

1. Crie uma pasta `Localization/<idioma>.lproj` com as mesmas chaves.
2. Adicione o idioma em `CFBundleLocalizations`, no `build.sh`.
3. Crie o caso correspondente em `AppLanguage` (`Localization.swift`).

## Distribuição

Para o Gatekeeper aceitar o app em outros Macs, é preciso uma conta paga do Apple Developer Program.

```bash
# Uma vez: guarda as credenciais da notarização no keychain
xcrun notarytool store-credentials kirei --apple-id <email> --team-id <TEAMID>

# Build assinado, notarizado e grampeado
DEVELOPER_ID="Developer ID Application: <Nome> (<TEAMID>)" \
NOTARY_PROFILE=kirei \
scripts/build.sh --dmg
```

| Variável | Efeito |
|---|---|
| `DEVELOPER_ID` | Assina o app e o DMG com Developer ID e hardened runtime |
| `NOTARY_PROFILE` | Envia o DMG para notarização e grampeia o ticket (staple). Exige `DEVELOPER_ID`. |

Ordem de assinatura usada pelo `build.sh`:

1. `DEVELOPER_ID`, se definido (distribuição).
2. Certificado local **Kirei Local Signing**, se existir no keychain (desenvolvimento).
3. Assinatura ad-hoc (a permissão de Gravação de Tela é pedida a cada build).

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
