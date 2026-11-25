# Kirei (GPUI)

Reescrita multiplataforma do Kirei em Rust com [GPUI](https://www.gpui.rs/), o framework de interface do Zed. Começa pelo macOS. Windows e Linux vêm depois, com a mesma interface e módulos de captura por sistema.

Esta é a versão em uso. O app em Swift (`../Sources/Kirei`) fica no repositório como referência para portar o que falta (editor, anotações, histórico, gravação, OCR).

## Rodar

```bash
scripts/bundle-macos.sh            # build debug + monta e assina target/debug/Kirei.app
scripts/bundle-macos.sh --install  # build release e instala em /Applications/Kirei.app
```

Teste sempre pelo `.app`. Com `cargo run`, a permissão de Gravação de Tela fica com o terminal, não com o app.

O bundle usa o identificador `com.shiddo.kirei` e o certificado local "Kirei Local Signing" (veja o README principal), então a permissão de Gravação de Tela continua valendo depois de recompilar.

| Atalho | Ação |
|---|---|
| ⌥⇧1 | Capturar área (Esc cancela; clique sem arrastar captura a tela toda) |
| ⌥⇧3 | Capturar tela inteira |

O primeiro build leva alguns minutos, porque o GPUI compila muita coisa. Os seguintes são incrementais. O `runtime_shaders` dispensa o Xcode completo: basta o Command Line Tools.

## Estrutura

```
app/
├── src/
│   ├── main.rs           # barra de menus (tray-icon), atalhos (global-hotkey), fluxo de captura
│   ├── overlay.rs        # seleção de área: uma janela PopUp por tela, sobre a imagem congelada
│   ├── preview.rs        # janela de resultado (vai virar o editor)
│   ├── capture/
│   │   ├── mod.rs        # tipos independentes de plataforma (DisplayShot, Captured)
│   │   └── macos.rs      # ScreenCaptureKit
│   └── platform/
│       └── macos.rs      # ajustes de AppKit (sem ícone no Dock)
└── scripts/bundle-macos.sh
```

Para portar para outra plataforma, crie `capture/windows.rs` e `capture/linux.rs` com a mesma função `freeze_displays()`. A interface não muda.

## Referência

O [Cap](https://github.com/CapSoftware/Cap) segue o mesmo modelo (GPUI + crates de captura por plataforma). Só os crates `scap-*` e `cap-camera*` dele são MIT. O resto é AGPL e serve apenas de referência de estrutura.
