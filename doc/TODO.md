# TODO — mini-rag

Ponto de retomada do projeto. Se você chegou aqui depois de um tempo longe,
comece pela seção [Como retomar](#como-retomar).

## Estado geral do roadmap

- [x] **Fase 0** — Setup: Axum + Tokio, Postgres + pgvector via Docker, SQLx CLI, `/health`
- [x] **Fase 1** — Modelagem e persistência: `documents`/`chunks`, migrations, CRUD básico
- [x] **Fase 2** — Ingestão de texto puro: upload, chunking semântico
- [x] **Fase 3** — Embeddings locais: `fastembed-rs`, persistência de vetores
- [x] **Fase 4** — Busca por similaridade: endpoint `/query`, índice HNSW
- [ ] **Fase 5** — Integração com LLM (em andamento — veja abaixo)
- [ ] **Fase 6** — Robustez: `thiserror`, validação, rate limiting, logging
- [ ] **Fase 7 (bônus)** — Suporte a PDF, DOCX
- [ ] **Fase 8** — Documentação final: diagrama de arquitetura, decisões

## Fase 5 — Integração com LLM

Plano completo e detalhes técnicos: [`doc/Fase-5.md`](./Fase-5.md)

### Progresso

- [x] **L0** — Decisão de provider → **Ollama local** + HTTP puro com `reqwest`
- [x] **L1** — Conceito da etapa de geração (fundo conceitual)
- [x] **L2** — Instalar Ollama e baixar modelo PT-BR → `qwen2.5:3b`
- [x] **L3** — `src/llm.rs`: cliente `reqwest` + chamada `/api/chat`
- [x] **L4** — Construção do prompt RAG (contexto + fontes)
- [x] **L5** — Resposta estruturada (`format: "json"`) + sources
- [ ] **L6** — Integração no `/query` (orquestração)
- [ ] **L7** — Revisão final da fase + ponte para Fase 6

## Bug conhecido (Fase 6 ou antes)

- [ ] Corrupção intermitente de espaços em chunks ("Para produtos" → "Paraprodutos").
      Não bloqueia a busca vetorial, mas degrada o texto retornado ao usuário.
      Ver pendência no `README.md`.

## Como retomar

1. **Ambiente**: `docker compose up -d` (Postgres). Verificar se o Ollama está
   rodando: `ollama list` / `curl localhost:11434/api/tags`. Se não, `ollama serve`.
2. **Banco**: `sqlx database create && sqlx migrate run`.
3. **App**: `cargo run` e teste `curl localhost:3000/health`.
4. **Próximo passo**: abra `doc/Fase-5.md` e continue na primeira caixa
   desmarcada de `Progresso`.
5. **Contexto essencial** (resumo rápido): API Axum em `src/main.rs` +
   `src/handlers.rs`; embeddings locais ONNX em `src/embedding.rs` (1 instância
   atrás de `Arc<Mutex<>>`, `spawn_blocking`); chunking em `src/chunking.rs`
   (parágrafo → sentença, máx. 256 tokens); persistência/busca pgvector em
   `src/db.rs` (`search_similar_chunks` retorna `distance: f64`, converter para
   similarity `1.0 - distance` na API).
