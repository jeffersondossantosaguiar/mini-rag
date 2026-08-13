# Fase 5 — Integração com LLM (plano de aula)

> Status: **em andamento** — progresso marcado no `TODO.md`.
> Nome desta fase no roadmap do README: "Integração com LLM: montagem de prompt
> com contexto recuperado, chamada à API, resposta com fontes".

## Decisões já tomadas

| Item | Decisão | Por quê |
|---|---|---|
| Provider | **Ollama (local)** | Gratuito, roda local (faz parte do espírito do projeto, igual embeddings locais), sem chave de API |
| Modelo | **`qwen2.5:3b`** | Cabe folgado na RAM (15GB total, ~7GB livres); 7b ficaria no limite
| Chamada | **HTTP puro com `reqwest`** | Aprender Rust de verdade: ver request, headers, status, parsing com serde. SDK esconde isso |
| Endpoint Ollama | `/api/chat` | Formato de mensagens (`system`/`user`), mais próximo do padrão de mercado |
| Saída estruturada | `format: "json"` no body | Força o modelo a responder JSON → parseável com serde |

## Aula 0 — Escolha do provider (concluída)

**Conceito ensinado — como avaliar um provider:** qualidade no idioma (PT-BR),
custo (projeto de estudo = quase irrelevante), ergonomia em Rust, saída
estruturada, fricção de setup. Importante: **input e output tokens são cobrados
separados**; output é ~3–5× o input. Chamadas RAG são input-dominadas (o
contexto recuperado vai no prompt).

Comparados (preços de ago/2026, cloud): Claude Haiku 4.5 ($1/$5) e Sonnet 5
($2/$10); OpenAI GPT-4.1-mini ($0.40/$1.60) e GPT-4.1 ($2/$8); Ollama = grátis.

**Quiz da L0 (respostas):**
1. Qual lado da cobrança domina em RAG? → **Input** (contexto é ~5× a resposta;
   apesar de output custar mais por token, o volume de input vence).
2. SDK oficial ou HTTP na mão? → **HTTP na mão** — você é dono do request,
   headers, erros e parsing.
3. A escolha do provider muda chunking/embedding? → **Não** — a recuperação é
   provider-agnostica. Só `src/llm.rs` muda.

## Aula 1 — A etapa de geração (concluída)

**Conceito:** a geração é o último passo do RAG: query → recuperar → **montar
prompt → chamar LLM → parsear resposta**. Termos novos: *context window*
(janela de contexto), papéis `system` vs `user`, `temperature`, `max_tokens`,
e por que o contexto injetado precisa ser cortado para caber na janela.

**Ligação com o que já sabemos:** o mesmo cuidado que tivemos com
`MAX_CHUNK_TOKENS = 256` na ingestão (embedding) se aplica aqui — não podemos
injetar contexto sem limite, senão estoura a janela do modelo.

**Coberto na aula (resumo):**
- Pipeline completo: ingestão → recuperação → **geração** (prompt → LLM → parse).
  A partir da Fase 5 a API passa a devolver uma resposta em linguagem natural.
- Conceitos: *context window* (teto duro — injetar além dela = erro ou truncamento),
  papéis `system`/`user`/`assistant`, `temperature` (baixa ~0.2 para RAG —
  reduz aleatoriedade, mas NÃO elimina alucinação; o prompt é o contrato),
  `max_tokens` (orçamento da resposta).
- Por que cortar o contexto: razão dura (janela) + razões suaves (custo, rede, qualidade).
- Ollama: `/api/chat` (mensagens com roles) em vez de `/api/generate`.
- Truque para a Aula 5: `format: "json"` força saída parseável com serde
  (não afeta o conhecimento do modelo, só o formato).

**Quiz respondido (nota: erros fazem parte — releia se precisar):**
1. Cortar contexto → custo/rede/qualidade, e o motivo principal: a janela é um limite duro.
2. `temperature` baixa → determinismo/grounding; mas não garante zero alucinação.
3. Endpoint `/api/chat`; papel = `system`.
4. `format: "json"` = parseabilidade, não conhecimento.

## Aula 2 — Instalar Ollama e escolher modelo PT-BR

Objetivo: deixar `ollama serve` rodando e um modelo baixado.

- [x] Instalar Ollama (v0.32.9) — via script oficial (`curl ... | sh`), systemd ativo
- [x] `ollama pull qwen2.5:3b` — **escolhido: 3b (~2GB)**
      Motivo: 15GB RAM total (~7GB livres) → 3b cabe folgado e responde rápido;
      7b (~4.7GB) ficaria no limite com risco de swap. Qualidade/conceitos não
      mudam com o tamanho; upgrade (`ollama pull qwen2.5:7b`) é 1 comando depois.
- [x] Testar com `curl` cru antes de escrever Rust — OK, resposta `"Oi!"`.
      Lições: `done_reason: "stop"`, `prompt_eval_count`/`eval_count` (tokens
      in/out), `total_duration` com cold start (~81s, `load_duration` 37s) vs
      modelo quente (~0.75s).
- [x] Testar JSON mode (`"format": "json"`) — OK, `content` virou JSON válido:
      `{"answer": "Brasília"}`. Base da Aula 5.

**Conceito:** o que é quantização e por que o tamanho do modelo (3B/7B = bilhões
de parâmetros) afeta qualidade vs uso de RAM.

## Aula 3 — `reqwest` + cliente Ollama (`src/llm.rs`)

**Conceito:** por que `reqwest` (client HTTP ergonômico por cima do `hyper`).
Forma de um POST com corpo JSON: URL, headers, body, `error_for_status`.

**Código:**
- [x] Adicionar `reqwest` 0.13 (com feature `json`) ao `Cargo.toml`
- [x] Criar `src/llm.rs` com `LlmClient` (`reqwest::Client` + `base_url` + `model`)
- [x] Função async `chat()` que monta o body do `/api/chat`, POSTa e retorna `message.content`
- [x] Registrar `mod llm;` no `main.rs` + campo `llm` em `AppState`
- [x] Config por env: `LLM_BASE_URL` / `LLM_MODEL` no `.env` (usuário antecipou Fase 6)
- [x] Revisão: structs `Serialize` (request) e `Deserialize` (response), `.json()` nos dois sentidos,
      `&str` vs `String` (ownership), `'static`, clone do `self.model` (não move de `&self`)

**Decisão de design:** `chat(messages: Vec<ChatMessage>)` é "burro" (envia qualquer coisa);
`build_rag_prompt(chunks, question)` é "esperto" (decide o que dizer). Separação de responsabilidades.

## Aula 4 — Construção do prompt RAG

**Conceito:** engenharia de prompt para RAG — injetar chunks recuperados,
instruir "responda apenas com base no contexto", marcar fontes, cortar o texto
injetado.

**Código:**
- [x] `SYSTEM_PROMPT` const em PT-BR (responder só do contexto, citar `[Fonte N]`, 1-3 sentenças)
- [x] `build_rag_prompt(chunks, question)`: message `system` + message `user`
      com `Contexto:\n...` (marcadores `[Fonte N]`) + `Pergunta: ...`
- [x] Orçamento de tokens: `MAX_PROMPT_TOKENS = 2048`, contagem cumulativa com
      `split_whitespace().count()` (proxy de tokens), `break` quando estourar
      (chunks já vêm ordenados por similaridade)
- [x] Revisão: enumerar `[Fonte N]` (index + 1); bug de lógica pego — `used_tokens`
      não era incrementado no loop (compilador não pega erro de lógica, só de tipo)

## Aula 5 — Resposta estruturada + fontes

**Conceito:** serde `Deserialize` na resposta, forçar JSON com `format: "json"`,
mapear `[Fonte N]` de volta para ids de chunk/documento.

**Ponto de partida da Aula 5 (onde paramos):**
1. Tornar `ChatMessage` `pub` (está em assinatura `pub fn` → warning de privacidade).
2. Mudar `chat()` para retornar o JSON cru (`body.message.content` já é o JSON do
   `format: "json"` — o modelo responde `{"answer": "...", "sources": [...]}` dentro
   dessa string). Revisar se `chat()` deve retornar `String` crua e um parser separado
   decodificar.
3. Structs `Answer`/`Sources` com `Deserialize` para o shape que o modelo responder.

**Código:**
- [ ] Structs `Answer` (campos da resposta JSON do Ollama)
- [ ] Parse da resposta + caso de erro (JSON malformado)
- [ ] Extração das fontes citadas e junção com os chunks recuperados
- [ ] Revisão: quiz de serde + exercício "e se o LLM devolver lixo?"

## Aula 6 — Integração no `/query`

**Conceito:** orquestração — o handler vira: busca → prompt → geração → resposta.
Novos caminhos de erro (Ollama fora do ar, contexto vazio).

**Código:**
- [ ] Refatorar `query_handler` (`src/handlers.rs:134`) para retornar `{ answer, sources }`
- [ ] Tratar erros: Ollama indisponível, nenhum chunk recuperado, JSON inválido
- [ ] Revisão: rastrear um request completo do início ao fim

## Aula 7 — Revisão final

- [ ] Quiz de revisão da fase inteira
- [ ] Atualizar `README.md` (decisões técnicas + marcar Fase 5 como feita)
- [ ] Ponte para a Fase 6 (robustez: `thiserror`, validação, rate limiting)

---

## Notas de contexto (para retomar sem perder fio)

- **Arquitetura atual**: `main.rs` monta o `Router` e injeta `AppState`
  (`pool: PgPool` + `embedder: Embedder`). Handlers em `handlers.rs`.
- **Recuperação já pronta**: `db::search_similar_chunks` devolve
  `ChunkSearchResult { chunk_id, document_id, content, distance: f64 }`.
  Na API, `similarity = 1.0 - distance` (ver gotcha f32/f64 no README).
- **Frase de resposta da API (antes da Fase 5)**: `{ results: [...] }`.
  Depois da Fase 5 deve virar `{ answer, sources: [...] }`.
- **Gatilhos de erro já existentes**: ingestão falha se chunk não gera embedding;
  query falha se embedding falha ou busca falha.
