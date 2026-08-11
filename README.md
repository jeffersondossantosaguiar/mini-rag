# mini-rag

API de Retrieval-Augmented Generation (RAG) construída em Rust, como projeto de estudo de systems programming, integração com IA e persistência com pgvector.

Parte da série `mini-*` de projetos from-scratch focados em entender profundamente os fundamentos por trás de ferramentas que uso no dia a dia.

## Stack

- **Runtime/Web**: Tokio + Axum
- **Banco de dados**: PostgreSQL + extensão `pgvector`
- **Data layer**: SQLx (raw SQL com verificação em compile-time)
- **Embeddings**: `fastembed-rs` (inferência local via ONNX, CPU)
- **LLM**: API da Anthropic/OpenAI para geração de resposta (RAG)
- **Observabilidade**: `tracing` + `tracing-subscriber`

## Motivação

Este projeto existe para aprender, na prática, os conceitos por trás de um pipeline de RAG completo: chunking, embeddings, busca vetorial e prompt engineering — sem depender de frameworks de alto nível (tipo LangChain) que escondem esses detalhes.

## Roadmap

- [x] **Fase 0** — Setup: projeto Axum + Tokio, Postgres com pgvector via Docker Compose, SQLx CLI, endpoint `/health`
- [x] **Fase 1** — Modelagem e persistência: schema `documents`/`chunks`, migrations, CRUD básico
- [x] **Fase 2** — Ingestão de texto puro: endpoint de upload, chunking semântico
- [x] **Fase 3** — Embeddings locais: integração com `fastembed-rs`, persistência de vetores
- [x] **Fase 4** — Busca por similaridade: endpoint de query, índice IVFFlat/HNSW no pgvector
- [ ] **Fase 5** — Integração com LLM: montagem de prompt com contexto recuperado, chamada à API, resposta com fontes
- [ ] **Fase 6** — Robustez: tratamento de erros (`thiserror`), validação, rate limiting, logging estruturado
- [ ] **Fase 7 (bônus)** — Suporte a outros formatos: PDF, DOCX
- [ ] **Fase 8** — Documentação final: diagrama de arquitetura, decisões técnicas

## Como rodar

```bash
docker compose up -d
sqlx database create
sqlx migrate run
cargo run
```

Health check: \`curl localhost:3000/health\`

## Decisões técnicas

Decisões de arquitetura e seus trade-offs serão documentadas aqui conforme o projeto avança (ex: por que SQLx e não SeaORM, por que embeddings locais e não via API, IVFFlat vs HNSW, etc.)# mini-rag

### Fase 2 — Chunking e persistência

**Chunking semântico (parágrafo → fallback sentença)**
Ao invés de cortar por N caracteres fixos, o texto é dividido primeiro por parágrafo (`\n\n`). Se um parágrafo ultrapassa o limite (1000 caracteres, provisório até a Fase 3), ele é subdividido por sentença, agrupando sentenças até chegar perto do limite. Isso preserva unidades de sentido inteiras em vez de cortar no meio de uma frase.

_Limitação conhecida_: o split por sentença usa pontuação (`.`, `!`, `?`) de forma ingênua, sem tratar abreviações (ex: "Dr. Silva" seria interpretado como fim de frase). Aceito conscientemente por agora — resolver isso exigiria uma lib de segmentação de texto (ex: `unicode-segmentation`) ou um modelo de NLP dedicado.

**Caracteres vs. tokens**
O limite de tamanho do chunk é medido em caracteres nesta fase, como aproximação. Na Fase 3, a mesma lógica de agrupamento passa a medir tokens reais, usando o tokenizer do modelo de embedding (`fastembed-rs`) — a estrutura do chunking (onde cortar) não muda, só a régua usada para decidir "cabe ou não cabe" no limite.

**Transação para atomicidade**
`create_document_with_chunks` insere o documento e todos os seus chunks dentro de uma transação (`pool.begin()` / `tx.commit()`). Se qualquer insert falhar no meio do processo, a transação sofre rollback automático e nada fica salvo parcialmente.

_Trade-off pendente_: os chunks ainda são inseridos um a um em um loop (N round-trips até o banco), não em batch. Transação resolve consistência, não performance — batch insert (via `UNNEST`) fica como otimização para a Fase 6.

### Fase 3 — Embeddings locais

**spawn_blocking para inferência CPU-bound**
A geração de embeddings é síncrona e consome CPU (não tem `.await` interno). Rodar isso direto num handler async travaria a thread do runtime do Tokio, impedindo outras requests de serem processadas. `tokio::task::spawn_blocking` move essa carga para uma thread pool dedicada, análogo ao uso de Worker Threads no Node para não bloquear o event loop.

**Arc<Mutex<TextEmbedding>> — uma instância compartilhada**
O modelo carregado em memória é envolto em `Arc<Mutex<>>` para ser compartilhado entre requests com segurança. Isso significa que, hoje, apenas uma inferência roda por vez — requests concorrentes de embedding esperam a vez (fila). Por isso a geração de embeddings dos chunks durante a ingestão é feita sequencialmente, não em paralelo: paralelizar as chamadas não traria ganho real, já que todas disputariam o mesmo lock.

**Contagem real de tokens (ao invés de caracteres)**
O chunking mede o tamanho contra o limite real de sequência do modelo (256 tokens do `all-MiniLM-L6-v2`), usando o tokenizer real via crate `tokenizers`. O tokenizer é carregado de um arquivo local (`assets/tokenizer.json`) em vez de baixado via `from_pretrained` em runtime — evita depender de disponibilidade do Hugging Face Hub toda vez que o servidor sobe (nasceu de um erro real de 401 ao tentar baixar em runtime).

## Fase 4 — Busca por Similaridade

Endpoint `POST /query`: recebe uma pergunta em texto puro, gera o embedding via
o mesmo `Embedder` usado na ingestão, e busca os chunks mais similares no
Postgres via pgvector (`<=>`, cosine distance).

### Decisões técnicas

**Índice: HNSW em vez de IVFFlat**
HNSW não exige treino prévio com dados representativos (IVFFlat precisa de
uma amostra pra construir os clusters via k-means, ruim pra ingestão
incremental como a nossa). Trade-off: construção do índice mais lenta e mais
uso de RAM, mas melhor recall/latência de busca e não degrada com inserts
sequenciais ao longo do tempo — que é exatamente o nosso padrão de uso.

**Verbo HTTP: POST em vez de GET numa operação de busca**
Tecnicamente quebra a convenção REST (GET = leitura, POST = escrita), mas o
payload de entrada é texto livre potencialmente longo — query string tem
limite prático de tamanho e é frágil pra texto com acentuação/caracteres
especiais. Mesma escolha usada por APIs de busca semântica de mercado
(Elasticsearch, Algolia).

**Exposição de `similarity` em vez de `distance`**
O pgvector retorna cosine _distance_ (0 = idêntico, 2 = oposto) — pouco
intuitivo pra quem consome a API ("quanto menor, melhor" é contraintuitivo).
Convertemos pra similarity (`1.0 - distance`) na fronteira handler↔API,
mantendo o banco/índice trabalhando com distance (mais natural pro `ORDER BY`
e pro índice).

**Migrations nomeadas por schema, não por feature**
A partir dessa fase, adotado o padrão `<timestamp>_<verbo>_<o_que>.sql`
(ex: `add_hnsw_index_chunks_embedding.sql`). O nome descreve o que muda no
schema, não em qual fase/feature foi feito — facilita entender o histórico
do banco meses depois, sem depender de contexto do roadmap do produto.

### Gotcha: `f32` vs `f64` no resultado do pgvector

Os operadores `<=>` e `<#>` do pgvector retornam `double precision` (`f64`),
não `real` (`f32`). Usar `sqlx::query_as!` com override de tipo
(`"distance!: f32"`) **não gera erro de compilação** — o `!` desativa a
checagem de tipo do SQLx. Em runtime, isso causa um bug silencioso: o SQLx
decodifica os primeiros 4 bytes de um valor de 8 bytes como se fossem um
`f32` completo, retornando um número plausível mas matematicamente sem
relação com o valor real (reinterpretação binária, não conversão numérica).

Sintoma observado: valores de `similarity` sempre negativos e, mais revelador,
**idênticos independente do vetor de entrada** — sinal de que o resultado não
tinha relação com o vetor calculado, e sim com um artefato de decodificação.

Como foi encontrado: bisseção sistemática, eliminando camada por camada
(tokenizer/pooling → ingestão → serialização do bind → operador SQL puro)
até isolar com um teste de controle (vetor unitário conhecido `<#>` ele
mesmo, esperado `-1.0` exato) que o erro só aparecia via SQLx com macro,
nunca via `psql` direto — apontando pra decodificação, não cálculo.

Correção: usar `f64` no struct de retorno e no override de tipo
(`"distance!: f64"`), convertendo pra `f32` só na borda da API se necessário.

**Lição geral**: overrides de tipo (`!:`) no SQLx desativam validação em
compile-time — qualquer uso deles em coluna calculada (não vinda direto de
uma coluna tipada da tabela) merece um teste de sanity check contra um valor
matematicamente conhecido antes de confiar no resultado.

### Pendência conhecida (não bloqueia a Fase 4)

Identificada corrupção intermitente de espaços em texto de chunks após
chunking (ex: "Para produtos" → "Paraprodutos"). Ainda não investigado a
fundo — aparenta ser intermitente, não sistemático (alguns chunks do mesmo
documento vêm corretos, outros não). Não afeta a lógica de busca vetorial em
si (embeddings são gerados corretamente a partir do texto, corrompido ou
não), mas degrada a qualidade do texto retornado ao usuário. Investigar na
Fase 6 (Robustez) ou antes, se atrapalhar a Fase 5.## Fase 4 — Busca por Similaridade

Endpoint `POST /query`: recebe uma pergunta em texto puro, gera o embedding via
o mesmo `Embedder` usado na ingestão, e busca os chunks mais similares no
Postgres via pgvector (`<=>`, cosine distance).

### Decisões técnicas

**Índice: HNSW em vez de IVFFlat**
HNSW não exige treino prévio com dados representativos (IVFFlat precisa de
uma amostra pra construir os clusters via k-means, ruim pra ingestão
incremental como a nossa). Trade-off: construção do índice mais lenta e mais
uso de RAM, mas melhor recall/latência de busca e não degrada com inserts
sequenciais ao longo do tempo — que é exatamente o nosso padrão de uso.

**Verbo HTTP: POST em vez de GET numa operação de busca**
Tecnicamente quebra a convenção REST (GET = leitura, POST = escrita), mas o
payload de entrada é texto livre potencialmente longo — query string tem
limite prático de tamanho e é frágil pra texto com acentuação/caracteres
especiais. Mesma escolha usada por APIs de busca semântica de mercado
(Elasticsearch, Algolia).

**Exposição de `similarity` em vez de `distance`**
O pgvector retorna cosine _distance_ (0 = idêntico, 2 = oposto) — pouco
intuitivo pra quem consome a API ("quanto menor, melhor" é contraintuitivo).
Convertemos pra similarity (`1.0 - distance`) na fronteira handler↔API,
mantendo o banco/índice trabalhando com distance (mais natural pro `ORDER BY`
e pro índice).

**Migrations nomeadas por schema, não por feature**
A partir dessa fase, adotado o padrão `<timestamp>_<verbo>_<o_que>.sql`
(ex: `add_hnsw_index_chunks_embedding.sql`). O nome descreve o que muda no
schema, não em qual fase/feature foi feito — facilita entender o histórico
do banco meses depois, sem depender de contexto do roadmap do produto.

### Gotcha: `f32` vs `f64` no resultado do pgvector

Os operadores `<=>` e `<#>` do pgvector retornam `double precision` (`f64`),
não `real` (`f32`). Usar `sqlx::query_as!` com override de tipo
(`"distance!: f32"`) **não gera erro de compilação** — o `!` desativa a
checagem de tipo do SQLx. Em runtime, isso causa um bug silencioso: o SQLx
decodifica os primeiros 4 bytes de um valor de 8 bytes como se fossem um
`f32` completo, retornando um número plausível mas matematicamente sem
relação com o valor real (reinterpretação binária, não conversão numérica).

Sintoma observado: valores de `similarity` sempre negativos e, mais revelador,
**idênticos independente do vetor de entrada** — sinal de que o resultado não
tinha relação com o vetor calculado, e sim com um artefato de decodificação.

Como foi encontrado: bisseção sistemática, eliminando camada por camada
(tokenizer/pooling → ingestão → serialização do bind → operador SQL puro)
até isolar com um teste de controle (vetor unitário conhecido `<#>` ele
mesmo, esperado `-1.0` exato) que o erro só aparecia via SQLx com macro,
nunca via `psql` direto — apontando pra decodificação, não cálculo.

Correção: usar `f64` no struct de retorno e no override de tipo
(`"distance!: f64"`), convertendo pra `f32` só na borda da API se necessário.

**Lição geral**: overrides de tipo (`!:`) no SQLx desativam validação em
compile-time — qualquer uso deles em coluna calculada (não vinda direto de
uma coluna tipada da tabela) merece um teste de sanity check contra um valor
matematicamente conhecido antes de confiar no resultado.

### Pendência conhecida (não bloqueia a Fase 4)

Identificada corrupção intermitente de espaços em texto de chunks após
chunking (ex: "Para produtos" → "Paraprodutos"). Ainda não investigado a
fundo — aparenta ser intermitente, não sistemático (alguns chunks do mesmo
documento vêm corretos, outros não). Não afeta a lógica de busca vetorial em
si (embeddings são gerados corretamente a partir do texto, corrompido ou
não), mas degrada a qualidade do texto retornado ao usuário. Investigar na
Fase 6 (Robustez) ou antes, se atrapalhar a Fase 5.

## Melhorias futuras

- **Pool de múltiplas instâncias do modelo de embedding** — hoje existe apenas uma instância protegida por `Mutex`, o que serializa a geração de embeddings mesmo sob carga concorrente. Ter um pool de N instâncias do modelo permitiria paralelismo real na geração de embeddings, ao custo de mais memória RAM (cada instância carrega os ~90MB do modelo).
- **Suporte a múltiplos modelos de embedding** — permitir configurar/trocar o modelo de embedding (ex: um multilíngue focado em PT-BR, ou um maior/mais preciso) sem reescrever a lógica de negócio, exigiria abstrair `Embedder` atrás de uma interface e versionar a dimensão do vetor por modelo (hoje travada em `VECTOR(384)` na migration).
- **Batch insert de chunks** — hoje os chunks são inseridos um a um dentro da transação (N round-trips ao banco). Migrar para um único `INSERT` via `UNNEST` reduziria overhead de rede.
