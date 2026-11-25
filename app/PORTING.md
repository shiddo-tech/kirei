# Paridade com o Kirei em Swift

Checklist do port de `../Sources/Kirei` para o app GPUI.

Legenda:
- **[x] oráculo**: comparado com a saída do binário Swift (`scripts/oracle/run.sh`)
- **[x] teste**: coberto por `cargo test`
- **[x] tela**: conferido por captura da janela com estado injetado (`--edit`/`--show`)
- **[~]**: implementado, falta alguém usar de verdade (mouse, teclado, permissões)
- **[ ]**: não portado

Ferramentas de verificação:
- `scripts/oracle/run.sh`: 16 renders (2 imagens × 8 estilos) e a captura com rolagem contra o Swift
- `kirei --record-test out.mp4 [id-da-janela]`: grava 2 s de uma área ou de uma janela (precisa da permissão de Gravação de Tela no processo que roda o comando)
- `kirei --gif-test video.mp4`: gera o pôster e o GIF de um vídeo
- `cargo test`: modelos, traduções, OCR/padrões, exportação, histórico, gestos do editor (TestAppContext)
- `kirei --edit img.png` com `KIREI_UI_STATE=...` e `kirei --show settings|history|welcome|welcome-permission|welcome-denied|welcome-shortcuts|recording-controls|recording-preview <mp4>|thumbnail <img>`: abrem só a janela, sem atalhos globais, sem ícone na barra de menus, sem roubar o foco e com dados em pasta temporária

## 1. Modelos e renderizador
- [x] oráculo: `Style`, `Background`, `Art`, `Annotation` com JSON igual ao Codable do Swift (o Swift lê os estilos gerados)
- [x] oráculo: layout do canvas (tamanho, posição 3×3, proporções)
- [x] oráculo: fundos sólido, degradê, mesh legado, Art (mesh/linear/radial), imagem, imagem com desfoque, transparente
- [~] granulado: aleatório nos dois apps, conferido só no olho
- [x] oráculo: sombra em duas camadas, cantos contínuos, inset, borda fina, molduras macOS e navegador claro/escuro, assinatura
- [x] oráculo: seta, retângulo, contador, texto, marca-texto, pixelizar (grade e amostragem iguais ao CIPixellate)
- [x] teste: elipse e desfocar desenhados pelo editor (sem oráculo visual)
- [x] oráculo: texto via CoreText (fonte do sistema com fallback); ttf-parser nas outras plataformas
- [x] oráculo: auto-equilíbrio e cor da borda
- [x] teste: exportação PNG (DPI), JPEG, HEIC (ImageIO), @1x, nome do arquivo

## 2. Editor
- [x] tela: janela com canvas, barra de ferramentas, barra inferior e inspetor
- [x] teste: desenhar cada tipo, cancelar arraste pequeno, seleção por retângulo, ⇧, mover, ⌥-duplicar, redimensionar pela alça, guia de alinhamento no centro, recorte, desfazer/refazer
- [x] tela: alças de seleção, menu de contexto, popover de cor/espessura, inspetor (estilos, Arte/Cor/Imagem, Moldura, Layout, Avançado)
- [~] teclas V A R O H T N P B C, ⌫, Esc, setas, ⌘C/⌘S/⇧⌘S/⌘D/⌘A/⌘P/⌘Z/⇧⌘Z/⌘]/⌘[/⌘+/⌘−/⌘0/⌘9/⌥⌘I
- [~] edição de texto e do contador com duplo clique (campo de texto próprio, com IME)
- [~] vibração no trackpad ao encaixar nas guias
- [~] painel de cor do sistema (anotações e fundo)
- [~] estilos salvos (criar, aplicar, atualizar, apagar; o último estilo vira o padrão)
- [~] arrastar a miniatura para outros apps, compartilhar, salvar, salvar como, copiar (PNG + TIFF com tamanho Retina)
- [x] oráculo: OCR e ocultar dados sensíveis (mesmo texto e mesmos 3 trechos que o Swift)
- [~] fixar na tela (⌘P)
- [~] papel de parede como fundo (falta o caminho de captura quando o sistema não dá o arquivo)

## 3. Captura
- [~] área (arrastar; clique sem arrastar = tela)
- [~] tela sob o mouse
- [~] janela (Espaço ou clique na seleção, ⌥⇧2)
- [~] lupa com coordenadas na seleção
- [~] temporizador (3/5/10 s) e ocultar ícones da mesa
- [~] miniatura flutuante (arrastar, copiar, salvar, editar, fixar; some em 7 s sem o mouse em cima)
- [~] ⌘C rápido enquanto a miniatura está na tela
- [~] sons (voz "kirei", obturador, cópia) sintetizados como no Swift
- [~] flash e animação da captura voando até a miniatura (curva 0.3, 1.25, 0.5, 1 em 0,42 s). Falta o "pulo" de escala da miniatura ao aparecer
- [~] vários monitores: popups, overlay e tela inteira usam a origem real de cada tela (o gpui 0.2 informa todas as telas na origem). Só conferido na tela principal
- [~] depois da captura: abrir editor / copiar com estilo (com ocultação automática)
- [~] ocultar dados automaticamente ao abrir o editor

## 4. Ajustes
- [x] tela: janela com abas Geral, Atalhos e Sobre
- [~] atalhos configuráveis (gravar, limpar; códigos Carbon iguais aos do Swift)
- [~] ação após captura, som, histórico, pasta, formato, @2x, idioma (com reinício), abrir ao iniciar (SMAppService)
- [~] status da permissão de Gravação de Tela

## 5. Histórico e fixar
- [x] teste: histórico de 14 dias / 60 itens, JSON igual ao do Swift (mesma pasta)
- [x] tela: janela de histórico (estado vazio)
- [~] grade com miniaturas, duplo clique edita, arrastar, menu (editar, copiar, fixar, mostrar no Finder, apagar), limpar
- [~] imagens fixadas: mover, redimensionar com rolagem, opacidade, deixar cliques passarem, Esc, fechar pela barra de menus (o menu de contexto é desenhado dentro da janela fixada e fica cortado em imagens pequenas)

## 6. Texto
- [~] OCR para a área de transferência (⌥⇧4)
- [~] editar imagem da área de transferência (⌥⇧5) e "Abrir imagem…"

## 7. Rolagem e gravação
- [x] oráculo: costura da captura com rolagem (`--scroll-test` idêntico pixel a pixel)
- [~] interface da captura com rolagem (borda, painel "Role devagar", ⌥⇧7)
- [x] teste: gravação MP4 H.264 de uma área (60 fps, cursor, dimensões pares, sem as janelas do Kirei) e de uma janela sozinha (Espaço ou clique na seleção) com `--record-test`
- [x] teste: GIF (12 fps, até 960 px, loop infinito) e pôster com `--gif-test`
- [x] tela: painel depois da gravação (Salvar MP4, Salvar GIF, Copiar, arrastar, duplo clique abre)
- [x] tela: borda vermelha tracejada e controle com cronômetro e Parar (en, pt-BR, es)
- [~] item "Parar gravação" e ícone de parar na barra de menus (⌥⇧6); a gravação termina sozinha quando o sistema encerra a captura
- [~] caminho completo atalho → seleção → Parar → painel (não foi rodado de ponta a ponta)
- Diferença: usa `SCRecordingOutput`, que só existe no macOS 15 ou mais novo. No macOS 14 aparece "Não foi possível começar a gravar". O Swift usa AVAssetWriter e funciona no 14. A taxa de bits fica com o sistema (o Swift fixava `max(4 Mbps, w×h×6)`)

## 8. Resto
- [~] barra de menus com as ações portadas, abrir imagem, histórico, fechar fixadas, ajustes, sair
- [~] menus do app (Kirei, Arquivo, Editar, Visualizar) enquanto há janela aberta; ícone no Dock só nesse momento
- [~] HUD de mensagens
- [~] diagnósticos: pânicos gravados em `~/Library/Logs/Kirei` (o Swift usa MetricKit)
- [x] tela: onboarding em 3 passos (boas-vindas, permissão, atalhos), traduzido e no tema do sistema
- [x] tela: passo da permissão sem a permissão (pt-BR)
- [~] passo da permissão aberto por uma captura sem permissão, com "Tentar mesmo assim"
- [ ] bundle com traduções em `Resources/` (hoje as traduções vão embutidas no binário)
- [ ] README
