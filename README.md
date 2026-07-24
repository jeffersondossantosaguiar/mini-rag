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
- [ ] **Fase 3** — Embeddings locais: integração com `fastembed-rs`, persistência de vetores
- [ ] **Fase 4** — Busca por similaridade: endpoint de query, índice IVFFlat/HNSW no pgvector
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
