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
- **Escalas 1x, 2x e 4x** — `1x` remasteriza **sem aumentar** a resolução (roda o modelo 4x e reduz
  de volta com Lanczos).
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

1. **Abra uma ou várias imagens** (`Ctrl+O`, **Abrir** ou arraste). Com mais de uma, aparece a lista
   à esquerda; clique em um item para comparar.
2. Escolha o motor (**IA** ou **Rápido**), o **modelo** e a escala (**1x** / **2x** / **4x**).
3. Use **Remasterizar** (item selecionado) ou **Remasterizar todas**. Depois de pronto, selecione
   qualquer item da lista para ver original ↔ resultado.
4. Compare:
   - **Lado a lado** — mesma região nos dois painéis; zoom/pan sincronizados.
   - **Slider** — arraste a divisória para revelar original ↔ resultado.
   - Role o mouse para zoom, arraste para mover, duplo clique (ou **Ajustar**) reenquadra, **1:1** mostra 1 pixel do resultado por pixel de tela.
5. **Salvar** (`Ctrl+S`) salva o item atual; **Salvar todas** exporta todos os resultados para uma pasta.

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
├── composables/useImageJob.ts# invoke/listen dos comandos Tauri
└── components/
    ├── Toolbar.vue           # motor, modelo, escala, ações
    ├── ImageList.vue         # lista lateral com miniaturas e status
    ├── ComparisonViewer.vue  # comparação com zoom/pan sincronizados + slider
    └── ProgressBar.vue       # progresso via eventos
src-tauri/                    # backend Rust
├── src/commands.rs           # load_image, upscale_image, save_image, runtime_info, ...
└── src/processing/
    ├── mod.rs                # catálogo de modelos e resolução de caminhos
    ├── worker.rs             # orquestra o job, progresso e arquivo temporário
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
  com Lanczos; a 4x é a saída nativa do modelo.
- Tempos medidos (release, GPU 8 GB): x4plus 410×307 → 820×614 em **~1,4 s**; 1024×768 → 2048×1536
  em **~9 s**; x4v3 1024×768 → 4096×3072 em **~0,9 s**. Em CPU, x4plus 1024×768 → 4x leva ~62 s.
- Tile adaptativo: 256 px (padrão) ou 384 px em GPUs com ≥12 GB; `IMAGE_REMASTER_TILE` sobrescreve
  (512 px estoura 8 GB com x4plus). A sessão/arena CUDA é liberada quando o modelo ou o tamanho da
  imagem muda; se ainda faltar memória, o app tenta de novo com sessão nova e depois com blocos
  menores (192/128 px). O limite de arena é ~80% da VRAM total.
- **Limite**: ~6 MP de entrada, por causa da memória do resultado 4x.
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
