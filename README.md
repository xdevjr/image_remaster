# Image Remaster

Aplicação desktop em **Rust + Tauri v2 + Vue 3** para fazer upscale/remaster de imagens de baixa
resolução e comparar o resultado lado a lado com o original.

![fluxo](https://img.shields.io/badge/Rust-1.100-000?logo=rust) ![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB) ![Vue](https://img.shields.io/badge/Vue-3-42b883)

## Recursos

- **4 modelos de IA locais** (ONNX Runtime via `ort`), selecionáveis na interface:
  - **Rápido** — Real-ESRGAN x4v3 (4,7 MB, prévia veloz)
  - **Qualidade** — Real-ESRGAN x4plus (67 MB, melhor equilíbrio para fotos; padrão)
  - **Nítido** — 4x-UltraSharp (67 MB, mais textura, pode exagerar artefatos)
  - **Anime** — Real-ESRGAN anime 6B (18 MB, ilustrações/anime)
- **Lote** — adicione várias imagens; elas aparecem numa **lista clicável** e você compara cada uma.
- **Comparar várias imagens** lado a lado **sem remasterizar**: ative **Comparar**, marque as imagens
  na lista e escolha o layout — automático, em linha (horizontal), em coluna (vertical) ou grade de
  2 colunas (ex.: 2 em cima, 2 embaixo) — com zoom/pan sincronizados.
- **Fluxo em árvore** — ramifique a partir do original ou de qualquer nó; cada ramo gera resultados
  independentes. Ex.: `2x Nítido` com dois filhos (`1x Qualidade` e `1x Anime`) sem um afetar o
  outro. Cada nó usa a saída do nó pai (raízes usam o original), e o visualizador mostra todos os
  resultados, com opção de ocultar painéis/nós sem deixar de processá-los.
- **Presets de fluxo** — salve a árvore atual com um nome e aplique o mesmo fluxo em outras imagens.
- **Salvar resultado por painel** — clique com o botão direito em qualquer imagem gerada para salvar
  aquele resultado individualmente; o menu também permite ocultar o painel.
- **Escalas 1x, 2x e 4x** — a saída padrão é **1x**; `1x` remasteriza **sem aumentar** a resolução
  (roda o modelo 4x e reduz de volta com Lanczos).
- **Resolução alvo** — por nó, defina uma largura/altura final em vez do multiplicador. A proporção
  original é mantida; alvos menores/iguais à entrada apenas redimensionam (sem modelo), alvos maiores
  usam o modelo e reduzem para a resolução pedida.
- **Seleção de nós para salvar** — o ícone de disquete em cada nó marca quais nós o
  **Salvar todas → nós marcados** deve exportar.
- **Editor gráfico em árvore** — a imagem original fica no topo com miniatura e os nós são
  conectados por setas, com miniatura do resultado, entrada, tempo e **edição completa dentro do
  próprio cartão**: motor, escala/alvo, modelo, + Ramo, salvar (disquete), mostrar/ocultar (olho) e
  remover (o nó e todos os descendentes). O modal pode ser movido pelo cabeçalho e redimensionado
  pelas bordas/cantos.
- **Ocultar o original** — no modo de comparação, use o botão **Original** (olho) para tirar o painel
  original da visualização; os resultados continuam disponíveis.
- **Salvar todas configurável** — escolha entre só o resultado final, todos os nós/ramos, apenas os
  nós visíveis ou apenas os nós marcados.
- **Aceleração por GPU (CUDA)** opcional, com fallback automático para CPU.
- **Tiling com sobreposição** — evita estourar memória e emendas visíveis.
- **Comparação lado a lado** com zoom/pan sincronizados, ou **modo slider** arrastável.
- Abrir por diálogo, **arrastar e soltar**, ou linha de comando (várias imagens).
- Progresso por imagem, cancelamento e **salvar todas**.

## Pré-requisitos

- [mise](https://mise.jdx.dev/) (gerencia Rust e Bun)
- Linux: `webkit2gtk-4.1`, `gtk3` (Tauri v2)

```bash
mise install        # Rust 1.100.0 + Bun 1.4.5
bun install         # dependências do frontend
mise run model      # baixa os 4 modelos ONNX (~154 MB) para ./models
```

### GPU (opcional)

Requer GPU NVIDIA e CUDA 13/cuDNN 9. O script baixa os wheels oficiais do PyPI (~1,2 GB):

```bash
bash scripts/setup_cuda.sh      # instala em ~/.local/share/image-remaster/{cuda,cudnn}
bash scripts/toggle_gpu.sh off  # desliga (renomeia as pastas)
bash scripts/toggle_gpu.sh on   # religa
```

Sem essas libs o app usa CPU automaticamente. O provider CUDA do ONNX Runtime já vai no AppImage,
ao lado do executável (é de lá que o ORT o carrega).

## Desenvolvimento

```bash
mise run dev        # inicia Vite + Tauri em modo dev
```

## Build de produção (AppImage)

```bash
mise run build      # gera src-tauri/target/release/bundle/appimage/Image Remaster_0.1.0_amd64.AppImage
```

O build é feito em etapas (`scripts/build_appimage.sh`): compila sem empacotar, remove os providers
CUDA do diretório de saída (o linuxdeploy não consegue resolver as dependências deles), gera o
AppDir, e `scripts/finalize_appimage.sh` coloca os providers ao lado do executável e reempacota.
O AppImage final tem ~192 MB (4 modelos + provider + app).

## Uso

1. **Abra uma ou várias imagens** (`Ctrl+O`, **Abrir** ou arraste). A lista lateral abre sozinha ao
   adicionar imagens (e pode ser recolhida no botão **Lista**), então dá para adicionar mais imagens
   a qualquer momento, inclusive depois de remasterizar.
2. Monte o **Fluxo** da imagem selecionada (menu no topo). O editor abre em uma tela com a imagem
   original no topo; crie ramos a partir dela ou de qualquer nó. Cada cartão de nó é editável
   diretamente (motor, **saída** escala 1x/2x/4x ou resolução alvo, modelo, ramos, salvar e
   visibilidade). Cada imagem guarda sua própria árvore, exibida na lista lateral. Cada nó mostra a
   entrada, a miniatura do resultado e o tempo gasto.
3. Use **Remasterizar todas** no topo para processar a lista inteira, ou o ícone **▶** de um item
   para processar somente ele. Depois de pronto, selecione qualquer item da lista para ver o original
   + todos os nós do fluxo. Nós ocultos continuam sendo processados; use o olho no editor ou o menu
   de contexto do painel para mostrar/ocultar.
4. **Salvar**: o ícone **↓** de um item salva só aquela imagem; **Salvar todas** (topo) abre opções
   para salvar o resultado final, os nós marcados com o ícone de disquete, todos os nós/ramos ou
   apenas os visíveis;
   **Ctrl+S** salva a imagem selecionada; clique com o botão direito em um painel para salvar um nó
   específico.
5. **Presets**: no editor de fluxo, digite um nome e clique em **Salvar atual**; depois use
   **Aplicar** em qualquer imagem.
6. **Copiar/colar fluxo**: clique com o botão direito em um item da lista e escolha **Copiar fluxo**;
   em outro item, **Colar fluxo**. O fluxo copiado inclui todos os ramos.
7. Para comparar imagens **sem remasterizar**, clique em **Comparar**, marque os itens na lista e veja
   vários painéis com a mesma região e zoom sincronizados.
8. Compare:
   - **Lado a lado** — mesma região nos painéis; zoom/pan sincronizados (dois ou mais painéis).
   - **Original** — botão de olho para ocultar/mostrar o painel original.
   - **Slider** — arraste a divisória para revelar original ↔ resultado (quando há exatamente 2 painéis).
   - Role o mouse para zoom, arraste para mover, duplo clique (ou **Ajustar**) reenquadra, **1:1** mostra 1 pixel do resultado por pixel de tela.
6. **Salvar** (`Ctrl+S`) salva o resultado final do item atual; **Salvar todas** exporta todos os
   resultados para uma pasta.

Atalhos: `Ctrl+O` abrir · `Ctrl+S` salvar · `Espaço` alterna lado a lado/slider.

Linha de comando (aceita várias imagens; `--autorun` processa o lote):

```bash
image-remaster foto1.png foto2.png
image-remaster --autorun foto1.png foto2.png foto3.png
image-remaster --autorun --model nitido --scale 2 foto1.png foto2.png
```

O status bar mostra o modelo, a escala, o tempo e se rodou em **GPU** ou **CPU**.

## Arquitetura

```
src/                          # Vue 3 + Vite (UI)
├── App.vue                   # lista de jobs, lote, seleção, diálogos, atalhos
├── composables/
│   ├── useImageJob.ts        # invoke/listen dos comandos Tauri
│   └── useFlowPresets.ts     # presets de fluxo persistidos localmente
├── lib/flow.ts               # utilitários da árvore (ordem, entradas, nomes)
└── components/
    ├── Toolbar.vue           # fluxo, ações, comparação
    ├── FlowEditor.vue        # árvore de nós, ramos, presets e visibilidade
    ├── IconEye.vue           # ícone de olho aberto/fechado
    ├── ContextMenu.vue       # menu de contexto dos painéis (salvar/ocultar)
    ├── ImageList.vue         # lista lateral com miniaturas, status e checkboxes
    ├── ComparisonViewer.vue  # N painéis com zoom/pan sincronizados + slider
    └── ProgressBar.vue       # progresso via eventos
src-tauri/                    # backend Rust
├── src/commands.rs           # load_image, upscale_image, upscale_flow, save_image, ...
└── src/processing/
    ├── mod.rs                # catálogo de modelos e resolução de caminhos
    ├── worker.rs             # orquestra fluxo em árvore, progresso e arquivos temporários
    ├── esrgan.rs             # ONNX + tiling adaptativo, sessões por modelo, GPU
    └── classic.rs            # Lanczos3 + unsharp mask
```

## Privacidade e uso offline

**O processamento é 100% local.** Não há envio de imagens, telemetria ou analytics:

- Nenhum código do app faz requisições de rede — não há cliente HTTP nas dependências de runtime.
- Modelos e provider ficam em disco e são usados localmente; a telemetria do ONNX Runtime está
  desligada (`with_telemetry(false)`).
- O frontend usa apenas assets embutidos (sem CDN/fontes externas).
- Resultados temporários ficam em `~/.cache/dev.juniorgs.imageremaster/` até você salvar.

A rede só é usada em **preparação**: `mise install`, `bun install`, `cargo build`,
`mise run model` (baixa modelos) e, opcionalmente, `scripts/setup_cuda.sh` (libs da NVIDIA).
Nunca fotos. Depois disso o app funciona sem internet — validado com `unshare -rn`.

## Notas técnicas

- Entrada dinâmica `[1,3,H,W]` em RGB `[0,1]`, saída 4x. Escala 1/2 é obtida reduzindo a saída 4x
  com Lanczos; a 4x é a saída nativa do modelo. Em **resolução alvo**, o app encaixa o alvo na
  proporção original; se o alvo for menor/igual à entrada, apenas redimensiona (sem modelo).
- Tempos medidos (release, GPU 8 GB): x4plus 410×307 → 820×614 em **~1,4 s**; 1024×768 → 2048×1536
  em **~9 s**; x4v3 1024×768 → 4096×3072 em **~0,9 s**. Em CPU, x4plus 1024×768 → 4x leva ~62 s.
- Tile adaptativo: 256 px (padrão) ou 384 px em GPUs com ≥12 GB; `IMAGE_REMASTER_TILE` sobrescreve
  (512 px estoura 8 GB com x4plus). A sessão/arena CUDA é mantida entre nós/imagens do mesmo modelo
  (evita recarregar o modelo); só a troca de modelo libera as sessões. Se faltar memória, o app
  tenta de novo com sessão nova e depois com blocos menores (192/128 px). O limite de arena é ~80%
  da VRAM total.
- **Limite**: ~32 MP de entrada. A saída 4x ainda pode consumir bastante memória; o app usa tiling
  e fallback de blocos menores em caso de falta de VRAM.
- Imagens com alpha são compostas sobre fundo branco.
- Resultado fica em cache até salvar; o cache é limpo na inicialização.
- Variáveis úteis: `IMAGE_REMASTER_MODELS_DIR`, `IMAGE_REMASTER_MODEL`, `IMAGE_REMASTER_CUDA_DIR`,
  `IMAGE_REMASTER_CUDNN_DIR`, `IMAGE_REMASTER_TILE`, `IMAGE_REMASTER_CUDA_MAX_WORKSPACE`,
  `IMAGE_REMASTER_DEBUG`.

## Testes

```bash
cd src-tauri && cargo test
```

Cobrem o pipeline ONNX (pulado se o modelo faltar), o upscale clássico e a exportação PNG/JPEG/WebP.
