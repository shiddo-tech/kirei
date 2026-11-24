# Kirei by Shiddo

Capturas de tela bonitas e prontas para compartilhar, com um atalho. App nativo para macOS em Swift/SwiftUI, com ocultação automática de dados sensíveis (inclusive CPF e CNPJ).

## Instalar

```bash
scripts/build.sh --install   # compila, gera o ícone, assina e copia para /Applications
scripts/build.sh --dmg       # gera build/Kirei-<versão>.dmg (com fundo e layout; o Finder pede permissão de Automação na primeira vez)
```

A versão fica no arquivo `VERSION`. O número do build é gerado a cada compilação (data e hora).

### Distribuição (outros Macs)

Para o Gatekeeper aceitar o app em outros Macs, é preciso uma conta paga do Apple Developer Program:

```bash
# uma vez: guarda as credenciais da notarização no keychain
xcrun notarytool store-credentials kirei --apple-id <email> --team-id <TEAMID>

DEVELOPER_ID="Developer ID Application: <Nome> (<TEAMID>)" NOTARY_PROFILE=kirei scripts/build.sh --dmg
```

Com `DEVELOPER_ID`, o app é assinado com hardened runtime. Com `NOTARY_PROFILE`, o DMG é notarizado e grampeado (staple).

Só precisa das Command Line Tools (`xcode-select --install`). Requer macOS 14 ou mais recente.

Na primeira captura, autorize em **Ajustes do Sistema › Privacidade e Segurança › Gravação de Tela e Áudio do Sistema** e reabra o app.
O `build.sh` assina com o certificado local **Kirei Local Signing** (guardado no keychain de login). Assim a identidade do app não muda entre builds e a permissão continua valendo depois de recompilar.
Sem esse certificado, o build cai para a assinatura ad-hoc, e o macOS pede a permissão de novo a cada build.
Para remover o certificado: Acesso às Chaves › login › Meus Certificados › "Kirei Local Signing".

## Atalhos globais (configuráveis em Ajustes)

| Atalho | Ação |
|---|---|
| ⌥⇧1 | Capturar área (arraste para selecionar; Espaço ou clique escolhe uma janela; Esc cancela) |
| ⌥⇧2 | Capturar janela |
| ⌥⇧3 | Capturar a tela sob o mouse |
| ⌥⇧4 | Capturar texto (OCR direto para a área de transferência) |
| ⌥⇧5 | Editar a imagem copiada |
| ⌥⇧6 | Gravar a tela (área ou janela). O mesmo atalho para a gravação |
| ⌥⇧7 | Captura com rolagem: selecione a área e role para baixo devagar |

Em **Ajustes › Geral › Captura** há temporizador (3, 5 ou 10 s, clique na contagem para cancelar) e a opção de ocultar os ícones da mesa.

## Depois da captura

Por padrão aparece uma **miniatura flutuante** no canto da tela. Dá para arrastá-la para outro app, copiar, salvar ou abrir o editor (botão ou clique duplo). Ela some sozinha depois de 7 s, mas fica enquanto o mouse estiver em cima. Em **Ajustes › Geral** dá para trocar por "abrir o editor" ou "copiar direto".

Na seleção de área, uma lupa mostra os pixels ao redor do cursor, com as coordenadas.

No momento da captura há um flash e o som do Kirei: por padrão, uma voz robótica dizendo "kirei" (`Resources/kirei-voice.wav`, gerada por `scripts/make-voice.swift`). Em **Ajustes › Geral** dá para trocar pelo obturador. Os outros sons são sintetizados no próprio app. A imagem desliza até a miniatura.

- **⌘C rápido**: enquanto a miniatura pós-captura está na tela, ⌘C copia a imagem, mesmo com outro app em primeiro plano. Quando a miniatura some, o ⌘C volta a funcionar normalmente nos outros apps. Numa imagem fixada, ⌘C também copia.
- **Fixar na tela**: pela miniatura, pelo editor (⌘P) ou pelo histórico. A imagem fica por cima de tudo. Arraste para mover, role ou faça pinça para redimensionar, clique duas vezes para editar e use o clique direito para opacidade e "deixar cliques passarem". Esc fecha. As imagens fixadas também fecham pela barra de menus.
- **Histórico**: barra de menus › Histórico de capturas (⌘Y no app). Guarda os originais dos últimos 14 dias (até 60) em `~/Library/Application Support/Kirei/History`. Dá para desligar em Ajustes.
- **Gravação**: grava em MP4 (H.264, 60 fps, com cursor). Ao parar, a miniatura oferece salvar MP4, salvar GIF (12 fps, até 960 px) ou copiar. Durante a gravação, o ícone da barra de menus vira um botão de parar.

## Editor

- **Auto-equilíbrio**: remove bordas uniformes e centraliza o conteúdo
- **Fundo**: 16 degradês (com ângulo), 8 mesh, cores sólidas, cor personalizada, imagem, papel de parede ou transparente, com granulado opcional
- **Moldura**: janela do macOS ou navegador, claro ou escuro, e assinatura (@seunome) no canto
- **Layout**: margem, inset (preenchido com a cor da borda), cantos arredondados, sombra, proporção (1:1, 4:3, 16:9, 9:16, 4:5…) e posição 3×3
- **Anotações**: seta, retângulo, elipse, marca-texto, texto, contador numerado, pixelizar, desfocar e recorte. Dá para selecionar (clique, ⇧+clique, arrastar um retângulo ou ⌘A), mover, redimensionar pelos cantos (o texto também), editar texto com duplo clique, duplicar (⌘D ou ⌥+arrastar), trazer para frente ou enviar para trás, apagar (⌫), mudar cor e espessura e usar desfazer/refazer. Com uma ferramenta de desenho ativa, arrastar pela borda de um objeto já o move. Clique direito abre o menu do objeto. O número de um contador se edita com duplo clique: um número reinicia a sequência a partir dele, e um texto (uma letra, por exemplo) aparece sem ocupar um número. Apague o valor para voltar à numeração automática.
- **Ocultar dados sensíveis**: OCR (Vision) + detecção de e-mail, telefone, cartão (Luhn), CPF/CNPJ, IP, chaves de API e JWT, que são pixelizados
- **Copiar texto (OCR)**
- **Moldura fina**: uma borda de meio ponto separa a captura do fundo (pode desligar em Layout). Os cantos são contínuos (squircle) e a sombra tem duas camadas, uma de contato e uma ambiente.
- **Guias de alinhamento**: ao mover anotações, elas encaixam no centro e nas bordas da captura e das outras anotações, com vibração no trackpad. Segure ⌘ para mover livre.
- **Exportar**: copiar (⌘C), salvar na pasta padrão (⌘S), salvar como PNG/JPEG/HEIC (⇧⌘S), arrastar para outros apps e compartilhar. Em Ajustes dá para exportar em @1x
- **Estilos salvos**: cartões com miniatura no inspetor. Clique com o botão direito para atualizar ou apagar. O último estilo usado vira o padrão

Teclas no editor: V A R O H T N P B C (ferramentas), ⌫ apaga, setas movem (⇧ = 10 px), Esc desmarca.

## Estrutura

- `Sources/Kirei/Renderer.swift`: composição (layout, fundo, sombra) e desenho das anotações
- `CanvasView.swift`: pré-visualização interativa
- `ImageAnalysis.swift`: auto-equilíbrio, OCR e detecção de dados sensíveis
- `Capture.swift`: captura com ScreenCaptureKit (dentro do próprio app, usando a permissão do Kirei), exportação e HUD
- `SelectionOverlay.swift`: sobreposição de seleção de área ou janela
- `Settings.swift` / `SettingsView.swift`: preferências (janela com abas), presets e atalhos globais (Carbon)
- `FloatingPreview.swift`: miniatura flutuante depois da captura
- `Onboarding.swift`: boas-vindas, permissão de Gravação de Tela e atalhos
- `Chrome.swift`: vibrancy, Liquid Glass (macOS 26), área de arrastar da janela e miniaturas de estilos
- `Diagnostics.swift`: relatórios do MetricKit em `~/Library/Logs/Kirei`
- `CaptureFeedback.swift`: sons sintetizados, flash, animação até a miniatura, temporizador e vibração
- `PinnedImage.swift`: imagens fixadas na tela
- `History.swift`: histórico de capturas e sua janela
- `Recorder.swift`: gravação de tela (ScreenCaptureKit + AVAssetWriter) e exportação em GIF
- `ScrollingCapture.swift`: captura com rolagem (costura por hash das linhas)

Autoteste do renderizador: `.build/debug/Kirei --render-test entrada.png saida.png [estilo.json]`
Autoteste da captura com rolagem: `.build/debug/Kirei --scroll-test imagem-alta.png saida.png` (simula uma rolagem com cabeçalho e rodapé fixos)

## Idiomas

O app está em inglês, português (Brasil) e espanhol. Por padrão, segue o idioma do macOS. Dá para trocar em **Ajustes › Sistema › Idioma** (é preciso reiniciar o app).

- No código, os textos usam `L("English text")`. A chave é o próprio texto em inglês, então uma tradução que faltar aparece em inglês.
- As traduções ficam em `Localization/<idioma>.lproj/Localizable.strings`, e o `scripts/build.sh` copia essas pastas para dentro do app.
- Para incluir um idioma, crie uma nova pasta `.lproj` com as mesmas chaves, adicione o idioma em `CFBundleLocalizations` no `build.sh` e crie o caso correspondente em `AppLanguage` (`Localization.swift`).
