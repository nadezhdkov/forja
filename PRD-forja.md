# PRD — forja

**Produto:** `forja` — CLI para acelerar o trabalho diário com git e GitHub
**Autor:** (preencher)
**Versão do documento:** 3.0
**Data:** Agosto/2026
**Status:** Rascunho para revisão
**Substitui:** PRD v2 (`forja`, escopo de setup de ambiente) e v1 (`envcli`)

> **Nota sobre o nome:** `forja` é nome de trabalho, pendente de validação
> (§17, QA-01). No jargão da indústria, *forge* designa justamente plataformas de
> hospedagem git (GitHub, GitLab, Forgejo, Codeberg) — o nome é descritivo do
> domínio, não decorativo.

---

## 1. Resumo executivo

`forja` é uma CLI em Rust que colapsa as sequências repetitivas de git e GitHub
em comandos únicos, seguros e previsíveis.

O produto tem dois pilares:

1. **Fluxos.** Comandos que substituem sequências de 3 a 6 passos que o
   desenvolvedor executa toda semana — sincronizar uma branch, limpar branches
   já mergeadas, abrir um PR, criar um repositório novo já conectado ao remoto.
   Esses fluxos atravessam a fronteira entre `git` e `gh`, que nenhuma das duas
   ferramentas cruza sozinha.
2. **Conformidade.** Um arquivo declarativo (`forja.toml`) que descreve como o
   ambiente git deve estar configurado, e comandos que verificam e aplicam essa
   configuração de forma idempotente.

Os dois pilares compartilham o mesmo arquivo de configuração: as preferências
declaradas (protocolo de clone, visibilidade padrão, branch principal) alimentam
tanto o setup quanto os fluxos.

---

## 2. Contexto e problema

Quem trabalha com git e GitHub todo dia executa as mesmas sequências
indefinidamente, e cada uma tem armadilhas conhecidas:

**Sincronizar uma branch com a principal.** `git fetch`, checar se há trabalho
não commitado, `git rebase origin/main`, resolver ou abortar, `git push`. Errar a
ordem custa um conflito desnecessário; errar o push custa um force-push
destrutivo.

**Limpar branches locais.** Depois de alguns meses, `git branch` lista trinta
branches, metade já mergeada e deletada no remoto. Limpar exige combinar
`git branch --merged`, `git fetch --prune` e um `xargs` que a pessoa copia do
Stack Overflow toda vez, torcendo para não apagar algo que importa.

**Abrir um PR.** `git push -u origin HEAD`, depois `gh pr create` com quatro
flags, depois abrir no navegador. Três comandos, dois contextos mentais
diferentes.

**Criar um repositório novo.** `gh repo create` com visibilidade e flags,
`git init`, primeiro commit, `git remote add`, push inicial. Cinco passos, e a
metade que envolve suas preferências pessoais (privado ou público? SSH ou
HTTPS?) é redigitada toda vez.

**Configurar uma máquina nova.** `user.name`, `user.email`, `init.defaultBranch`,
`pull.rebase`, e os aliases que a pessoa carrega há anos e reescreve de cabeça.

O padrão comum: **cada passo individual é trivial, a sequência não é.** A
sequência carrega ordem, condicionais, checagens de segurança e preferências
pessoais que hoje moram na memória muscular — e que se perdem ao trocar de
máquina ou ficam desatualizadas num alias esquecido.

---

## 3. Alternativas consideradas

| Alternativa | O que resolve | Por que não basta |
|---|---|---|
| `gh` (GitHub CLI) | Toda a API do GitHub, muito bem | Não toca no estado local do git. `gh pr create` não faz o push antes. A fronteira entre local e remoto continua sendo trabalho manual. |
| Aliases de git | Atalhos de comando único | Não expressam sequências com condicionais e checagens de segurança. Um alias que faz rebase não sabe abortar se a árvore está suja. E não são portáveis sem um mecanismo de sincronização à parte. |
| Funções de shell / scripts | Sequências com lógica | Presos a um shell específico, sem validação, sem dry-run, sem mensagem de erro decente. É o estado atual do problema, não a solução. |
| `lazygit`, `gitui` | Ergonomia excelente para git local | São TUIs interativas. Não são scriptáveis, não são declarativas, e não cobrem GitHub. Coexistem com `forja` em vez de competir. |
| `git-extras` | Coleção de subcomandos úteis | Só git, sem GitHub, sem configuração declarativa. Sobreposição parcial nos fluxos locais. |
| `chezmoi` / `dotbot` | Sincronização de dotfiles, incluindo `.gitconfig` | Gerenciam arquivos, não intenção. Cobrem o pilar de conformidade de forma mais crua, e nada do pilar de fluxos. |

### 3.1 Tese do produto

A lacuna que `forja` ocupa é específica e verificável: **nenhuma ferramenta única
executa sequências que atravessam git local e GitHub remoto, com checagens de
segurança e preferências declaradas.**

`gh` conhece o GitHub e ignora o repositório local. `git` conhece o local e
ignora o GitHub. Aliases e scripts conseguem colar os dois, mas sem validação,
sem portabilidade e sem segurança. `forja` é a camada que falta entre eles.

**Critério de falseamento:** se, seis meses após o M1, o autor perceber que usa
`forja` apenas para o setup inicial e nunca para os fluxos, a tese está errada —
e a resposta correta é reduzir o produto ao setup ou arquivá-lo em favor de
`chezmoi`. Registrado aqui para tornar essa avaliação possível depois.

---

## 4. Objetivos e não-objetivos

### 4.1 Objetivos

- **O1.** Reduzir sequências recorrentes de git e GitHub a um comando cada.
- **O2.** Tornar essas sequências seguras por padrão: nenhum fluxo deve
  conseguir destruir trabalho não publicado.
- **O3.** Permitir declarar preferências e configuração de ambiente em um arquivo
  versionável, e aplicá-las de forma idempotente e observável.
- **O4.** Falhar de forma clara e acionável: config inválida, dependência
  ausente, ou pré-condição de segurança não satisfeita.

### 4.2 Não-objetivos

- **N1.** Não é um front-end para git. `forja` não terá `forja commit`,
  `forja push` ou `forja status`. Envolver comandos de um passo só adiciona uma
  camada de indireção sem valor. Ver DD-07.
- **N2.** Não substitui `gh`. Para qualquer operação do GitHub que não faça parte
  de uma sequência, use `gh` diretamente.
- **N3.** Não gerencia credenciais. Autenticação com GitHub é 100% delegada ao
  `gh` já autenticado. Nenhum token entra no `forja.toml`, nunca. Ver DD-05.
- **N4.** Não instala nada — nem `git`, nem `gh`, nem runtimes. Verifica e
  reporta; a instalação é do usuário.
- **N5.** Não é uma TUI. `forja` é não-interativa por padrão e scriptável.
- **N6.** Não sincroniza nada entre máquinas por conta própria. O `forja.toml` é
  um arquivo — versione junto dos seus dotfiles.
- **N7.** Nenhuma telemetria.

---

## 5. Público-alvo

**Primário:** desenvolvedores que usam git e GitHub diariamente, já conhecem os
comandos subjacentes, e querem eliminar a repetição sem abrir mão de controle.
O usuário-alvo sabe o que um rebase faz — ele só não quer digitar a sequência
inteira pela milésima vez.

**Explicitamente fora:** quem está aprendendo git (uma abstração por cima
atrapalha o aprendizado), equipes querendo padronizar fluxo corporativamente
(isso é política de repositório e CI, não CLI local), e quem prefere interface
visual (use `lazygit`).

---

## 6. Casos de uso

- **CU-01 — Sincronizar.** Minha branch está atrás da `main`. Rodo `forja sync` e
  ela busca, valida que minha árvore está limpa, rebaseia e publica — abortando
  com mensagem clara se qualquer pré-condição falhar.
- **CU-02 — Limpar.** Depois de meses de trabalho, rodo `forja cleanup` e vejo a
  lista de branches locais já mergeadas e removidas no remoto, com confirmação
  antes de apagar.
- **CU-03 — Abrir PR.** Terminei a feature. Rodo `forja pr` e ela publica a
  branch, cria o PR com upstream configurado e me devolve a URL.
- **CU-04 — Criar repositório.** `forja repo new minha-api` cria no GitHub com a
  visibilidade e o protocolo que declarei, inicializa o local, conecta o remoto e
  publica o primeiro commit.
- **CU-05 — Máquina nova.** Clono meus dotfiles, rodo `forja setup`, e tenho git
  configurado com meu nome, email, branch padrão e aliases em um comando.
- **CU-06 — Verificar conformidade.** Suspeito que essa máquina está com config
  divergente. Rodo `forja setup --dry-run` e vejo exatamente o que difere, sem
  alterar nada.
- **CU-07 — Diagnóstico.** Algo falhou. `forja doctor` me diz que `git` está no
  PATH, `gh` não está autenticado, e o que fazer sobre isso.

---

## 7. Escopo

### 7.1 MVP

O MVP entrega **um comando de cada pilar** — o mínimo para validar a tese de §3.1
com uso real, não com hipótese.

| Comando | Descrição |
|---|---|
| `forja init` | Gera um `forja.toml` comentado |
| `forja show` | Exibe a config carregada e normalizada. Somente leitura. |
| `forja doctor` | Verifica dependências no PATH, versões mínimas, e autenticação do `gh` |
| `forja setup` | Aplica a seção `[git]` da config via `git config --global` |
| `forja sync` | fetch + validações + rebase na branch base + push |
| `forja cleanup` | Remove branches locais já mergeadas e remotos órfãos |
| `--dry-run` | Global. Mostra o que aconteceria, sem executar nada. |

Os dois fluxos do MVP (`sync`, `cleanup`) são **puramente locais** — usam apenas
`git`, sem depender de `gh` nem de autenticação. Isso mantém o MVP livre da
complexidade de API e auth, e ainda assim prova o pilar de fluxos.

**Critério de conclusão:** os sete itens funcionam ponta a ponta em Linux e
macOS, com testes de integração contra repositórios git reais e temporários, e o
autor usa `sync` e `cleanup` no trabalho real por duas semanas sem cair de volta
nos comandos manuais.

### 7.2 Fora do escopo no MVP

- Qualquer integração com `gh` (fica para a Fase 2)
- Perfis e múltiplos contextos de identidade
- `forja capture`
- Busca de config em múltiplos caminhos
- Verificação de runtimes
- Windows (deve compilar; comportamento não é testado nem garantido)

### 7.3 Fases posteriores

- **Fase 2 — GitHub.** `forja pr` (push + criar PR + upstream) e
  `forja repo new` (criar remoto + init local + primeiro push), ambos via `gh`.
  É aqui que a tese de §3.1 fica plenamente exercida.
- **Fase 3 — Identidade e contexto.** Perfis (`--profile work`), geração de
  diretivas `includeIf` por diretório, e `forja capture` (gera o TOML a partir do
  estado atual da máquina).
- **Fase 4 — Ambiente estendido.** Seção `[runtimes]` verificada pelo `doctor`
  (versão ativa de Java, Node etc. comparada com a declarada, **sem instalar** —
  apenas reporta a divergência e sugere o comando). Busca de config em
  `./forja.toml` → `~/.config/forja/config.toml`.
- **Fase 5 — Mais fluxos.** Definidos por observação do uso real, não por
  antecipação. Candidatos: `forja adopt` (checkout de PR alheio para revisão),
  `forja wip` (commit temporário padronizado).

**Regra de progressão:** nenhuma fase começa antes de a anterior estar completa,
testada e usada de verdade por pelo menos duas semanas. Ver R-01.

---

## 8. Especificação do `forja.toml`

### 8.1 Regras gerais

- TOML 1.0. O campo `version` no topo é obrigatório e identifica o schema.
- Chaves desconhecidas geram **aviso**, não erro (DD-06). Chaves obrigatórias
  ausentes geram erro (exit 2).
- Nenhum campo aceita segredo, token ou senha (N3, DD-05).
- Toda a config é **opcional para os fluxos**. `forja sync` funciona sem
  `forja.toml` algum, usando defaults sensatos. A config só personaliza.

### 8.2 Schema — MVP

#### Raiz

| Campo | Tipo | Obrigatório | Default | Validação |
|---|---|---|---|---|
| `version` | string | Sim | — | Deve ser `"1"` |

#### `[git]` — usado por `forja setup`

| Campo | Tipo | Obrigatório | Default | Validação |
|---|---|---|---|---|
| `user_name` | string | Sim | — | Não vazio |
| `user_email` | string | Sim | — | Contém `@`, não vazio |
| `default_branch` | string | Não | `"main"` | Nome de ref válido |
| `editor` | string | Não | (não aplica) | Não vazio se presente |
| `pull_rebase` | bool | Não | (não aplica) | — |

Campos opcionais ausentes não são aplicados — `forja` não escreve defaults
implícitos no gitconfig do usuário.

#### `[git.aliases]`

Tabela livre `string = string`. Nome deve casar `^[a-zA-Z0-9_-]+$`; valor não
vazio.

#### `[flow]` — usado por `sync` e `cleanup`

| Campo | Tipo | Obrigatório | Default | Descrição |
|---|---|---|---|---|
| `base_branch` | string | Não | detectado do remoto | Branch base para o rebase do `sync` |
| `strategy` | enum | Não | `"rebase"` | `"rebase"` ou `"merge"` |
| `auto_push` | bool | Não | `true` | Se `sync` publica após integrar |
| `protected_branches` | array\<string\> | Não | `["main", "master"]` | Nunca deletadas pelo `cleanup`, nunca rebaseadas pelo `sync` |

### 8.3 Schema reservado (fases posteriores, ignorado com aviso no MVP)

```toml
[github]                 # Fase 2
default_visibility       # "private" | "public" | "internal"
clone_protocol           # "ssh" | "https"
default_owner            # string
pr_draft                 # bool — abre PRs como rascunho por padrão

[profiles.<nome>]        # Fase 3 — sobrescreve campos de [git]

[runtimes]               # Fase 4 — verificação, nunca instalação
java = "21"
node = "22"
```

### 8.4 Exemplo completo

```toml
# forja.toml
version = "1"

[git]
user_name      = "Fulano de Tal"
user_email     = "fulano@example.com"
default_branch = "main"
editor         = "nvim"
pull_rebase    = true

[git.aliases]
st   = "status -sb"
lg   = "log --oneline --graph --decorate --all"
undo = "reset --soft HEAD~1"

[flow]
strategy            = "rebase"
auto_push           = true
protected_branches  = ["main", "develop"]
```

### 8.5 Política de versionamento do schema

- Mudanças aditivas (novo campo opcional) não incrementam `version`.
- Mudanças incompatíveis incrementam para `"2"`, e `forja` passa a rejeitar
  `version = "1"` com mensagem indicando exatamente o que mudou.
- `forja` nunca reescreve o arquivo do usuário sem `--write` explícito.

---

## 9. Contrato de interface (CLI)

### 9.1 Flags globais

| Flag | Efeito |
|---|---|
| `--config <caminho>` | Caminho do arquivo. Default: `./forja.toml` |
| `--dry-run` | Exibe o plano de execução; não executa nada |
| `--yes` / `-y` | Pula confirmações interativas (para uso em scripts) |
| `--verbose` / `-v` | Loga cada comando externo executado, com argumentos |
| `--quiet` / `-q` | Suprime saída não-essencial; erros seguem em stderr |
| `--json` | Saída estruturada; nenhuma prosa se mistura |
| `--version`, `--help` | Padrão |

`--quiet` e `--verbose` são mutuamente exclusivos (exit 2).

### 9.2 Códigos de saída

| Código | Significado |
|---|---|
| 0 | Sucesso (inclui `--dry-run`) |
| 1 | Falha de execução: um comando externo retornou erro |
| 2 | Erro de configuração ou de uso: arquivo inválido, flags conflitantes |
| 3 | Dependência externa ausente ou não autenticada |
| 4 | Pré-condição de segurança não satisfeita — o fluxo abortou de propósito |
| 130 | Interrompido pelo usuário (SIGINT) |

O código 4 é o mais importante do conjunto: distingue "a ferramenta quebrou" de
"a ferramenta se recusou a fazer algo perigoso". Um script de CI trata os dois
casos de forma diferente.

### 9.3 Convenções de saída

- Saída legível em **stdout**; erros e avisos em **stderr**.
- Cores desativadas quando stdout não é TTY, e sempre com `NO_COLOR` definida.
- Fluxos que alteram estado imprimem o plano antes de executar.

### 9.4 Exemplo — `forja sync`

```
$ forja sync

Branch atual: feature/login
Base:         origin/main (detectada)

  ✓ árvore de trabalho limpa
  ✓ branch não é protegida
  → git fetch origin
  ✓ 4 commits novos em origin/main
  → git rebase origin/main
  ✓ rebase concluído sem conflitos
  → git push --force-with-lease

feature/login sincronizada com origin/main (4 commits integrados).
```

### 9.5 Exemplo — aborto seguro

```
$ forja sync

Branch atual: feature/login

  ✗ árvore de trabalho suja (3 arquivos modificados)

Abortado antes de qualquer alteração. Commite ou guarde suas mudanças:
  git stash push -m "wip"

exit 4
```

Este formato é o contrato visual do produto; mudanças nele são mudanças de
interface e vão para o CHANGELOG.

---

## 10. Requisitos funcionais

**RF-01.** O sistema deve carregar a config de `--config` ou `./forja.toml`.
Arquivo ausente não é erro para comandos de fluxo — apenas para `setup` e `show`.

**RF-02.** A validação deve ocorrer integralmente antes de qualquer alteração, e
reportar **todos** os erros encontrados de uma vez, com campo e linha quando
possível.

**RF-03.** `forja init` gera um `forja.toml` comentado. Se o arquivo já existir,
recusa com exit 2, salvo `--force`.

**RF-04.** `forja show` não executa nenhuma alteração nem invoca comando externo
que mute estado.

**RF-05.** `forja doctor` verifica presença e versão mínima de `git`, presença de
`gh` (aviso, não erro, no MVP) e o status de autenticação do `gh` quando
presente. Reporta todas as verificações, aprovadas e reprovadas. Exit 3 se uma
obrigatória falhar.

**RF-06.** `forja setup` aplica cada campo de `[git]` e cada entrada de
`[git.aliases]` via `git config --global`. Campos ausentes da config não são
tocados.

**RF-07.** Antes de qualquer alteração, o sistema lê o estado atual e computa as
divergências. Itens já conformes não geram chamada de escrita.

**RF-08.** Com `--dry-run`, toda a lógica de RF-07 executa e o resultado é
exibido, sem nenhuma escrita.

**RF-09.** `forja sync` deve, nesta ordem: verificar que há um repositório git;
verificar que a árvore está limpa; verificar que a branch atual não está em
`protected_branches`; determinar a branch base; buscar do remoto; integrar
conforme `strategy`; publicar se `auto_push`. Qualquer verificação reprovada
aborta **antes de qualquer alteração**, com exit 4.

**RF-10.** `forja cleanup` deve listar as branches locais candidatas — mergeadas
na base e ausentes no remoto, excluindo as protegidas — exibir a lista e pedir
confirmação antes de deletar. Com `--yes`, pula a confirmação. Nunca deleta
branch não mergeada, mesmo com `--yes`.

**RF-11.** Em caso de falha de comando externo, o sistema exibe o comando exato,
seu stderr e seu código de saída, lista o que já foi aplicado antes da falha, e
sai com 1.

**RF-12.** Rodar `forja setup` repetidamente converge para o mesmo estado. A
segunda execução consecutiva reporta zero alterações pendentes.

---

## 11. Requisitos não-funcionais

**RNF-01 — Distribuição.** Binário único, sem runtime externo. Alvos oficiais:
`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`.

**RNF-02 — Desempenho.** `show` e `doctor` em menos de 50 ms. `setup` em menos de
300 ms para até 20 aliases (ver DD-04). Fluxos com rede são dominados pela
latência do remoto; a meta é que o overhead de `forja` sobre os comandos git
equivalentes fique abaixo de 50 ms.

**RNF-03 — Portabilidade.** Compila e passa nos testes em Linux e macOS sem
`cfg` de plataforma no código de domínio. Windows não é testado no MVP.

**RNF-04 — Tratamento de erro.** Nenhum `panic!`, `unwrap()` ou `expect()` em
caminho alcançável por entrada do usuário. Verificado por lint em CI.

**RNF-05 — Qualidade das mensagens.** Toda mensagem de erro contém o que falhou,
por que falhou, e o que fazer. Mensagem sem ação sugerida é bug.

**RNF-06 — Segurança de dados do usuário.** Nenhum fluxo pode causar perda de
trabalho não publicado. Ver DD-08 para as regras específicas.

**RNF-07 — Manutenibilidade.** Adicionar um novo fluxo deve exigir um módulo novo
implementando o trait de fluxo e uma linha de registro, sem alterar parsing, CLI
ou o executor de comandos.

---

## 12. Decisões de design registradas

### DD-01 — Conflito de config: sobrescrever

O TOML declara `user_email = "a@x.com"`, a máquina tem `b@y.com`.
**Decisão:** sobrescrever, sem perguntar — declarativo significa que o arquivo é
a fonte da verdade. **Mitigação:** `--dry-run` mostra o valor anterior, e a saída
normal registra a transição (`antigo → novo`). Nada é perdido silenciosamente.
**Rejeitado:** perguntar interativamente (quebra N5); pular valores já definidos
(inutiliza a ferramenta no caso de correção, que é o principal).

### DD-02 — Sem rollback no `setup`

Cada escrita de `git config` é independente e idempotente. Rollback custaria
complexidade desproporcional ao dano de um estado parcial, que se resolve rodando
o comando de novo. **Compensação obrigatória:** RF-11 exige listar o que foi
aplicado antes da falha. Estado parcial é aceitável; estado parcial silencioso
não é.

### DD-03 — Ler antes de escrever

Sempre computar o diff antes de aplicar, mesmo sem `--dry-run`. Habilita
`--dry-run` sem código duplicado, torna a saída informativa e evita escritas
desnecessárias. Custo desprezível.

### DD-04 — Batching de escritas do `setup`

Um `git config --global` por chave significa N processos (~5–15 ms cada).
**Decisão MVP:** um processo por chave, com RNF-02 calibrado para essa realidade.
**Otimização futura:** escrever via `gix-config`, se necessário. Adiado por
adicionar risco de corromper o gitconfig — delegar ao `git` é mais seguro.

### DD-05 — Credenciais fora do TOML, sempre

Quando a Fase 2 chegar, autenticação será 100% delegada ao `gh`. O `forja.toml`
nunca terá campo de token, nem opcional. **Justificativa:** o arquivo foi
projetado para ser versionado em dotfiles frequentemente públicos; um schema que
*permite* token vai eventualmente ter um token vazado nele.

### DD-06 — Chaves desconhecidas avisam, não falham

Permite que um `forja.toml` escrito para a Fase 4 seja lido por um binário do MVP
sem quebrar. Falhar forçaria arquivos diferentes por versão de binário entre
máquinas — exatamente o problema que a ferramenta existe para resolver.

### DD-07 — Não envolver comandos de um passo só

`forja` não terá `forja commit`, `forja push`, `forja status`. **Justificativa:**
envolver um comando de um passo adiciona indireção sem valor, cria uma segunda
sintaxe para algo que o usuário já sabe fazer, e infla a superfície de manutenção
indefinidamente. Um comando só entra no produto se substituir **três ou mais
passos** ou embutir uma checagem de segurança que o git não faz sozinho. Este é o
critério de admissão de qualquer fluxo futuro.

### DD-08 — Fluxos abortam, não improvisam

Regras invioláveis para qualquer comando de fluxo:

- **Nunca** `git push --force`. Quando um push após rebase for necessário, usar
  `--force-with-lease`, que falha se o remoto mudou.
- **Nunca** mexer na árvore de trabalho sem consentimento. Nada de `stash`
  automático — se a árvore está suja, abortar e dizer o que fazer.
- **Nunca** deletar branch não mergeada, mesmo com `--yes`.
- **Nunca** operar em branch listada em `protected_branches`.
- Conflito de rebase **não** é resolvido automaticamente: `forja` deixa o
  repositório no estado de conflito, explica a situação e sai com 4.

**Princípio:** diante de ambiguidade, a ferramenta para e devolve o controle. Uma
CLI que "resolve sozinha" um caso ambíguo em cima do repositório de alguém é uma
CLI em que não se confia.

---

## 13. Estratégia de testes

**Unitários:** parsing e validação (TOML válido, malformado, campos ausentes,
tipos errados, `version` incorreta, chaves desconhecidas); cálculo de diff de
config, incluindo o caso "tudo conforme"; lógica de seleção de branches do
`cleanup`.

**Integração:**

- **Config:** `git` respeita `GIT_CONFIG_GLOBAL`. Apontando-a para um arquivo
  temporário, dá para testar `setup` de ponta a ponta, executando o `git` real,
  sem tocar no gitconfig da máquina.
- **Fluxos:** criar repositórios git temporários com um remoto local (`git init
  --bare` num diretório temporário serve como "origin" perfeitamente). Isso
  permite testar `sync` e `cleanup` de verdade — inclusive divergência, conflito
  de rebase e branch protegida — sem rede e sem GitHub.
- **Segurança:** cada regra de DD-08 tem um teste dedicado que constrói o cenário
  perigoso e verifica que a ferramenta aborta com exit 4. Estes são os testes
  mais importantes da suíte.
- **Idempotência:** rodar `setup` duas vezes; a segunda reporta zero alterações.
- **Snapshot** da saída de `--dry-run`, para que mudanças de formato sejam
  intencionais e visíveis em review.

**CI:** build + `clippy -D warnings` + suíte completa em Linux e macOS a cada
push.

---

## 14. Distribuição

- **Primário:** `cargo install forja`.
- **Secundário:** binários pré-compilados nas releases do GitHub via
  `cargo-dist`, para os alvos de RNF-01.
- **Futuro:** tap do Homebrew, se houver adoção.
- **Licença:** MIT OR Apache-2.0 (dual, padrão do ecossistema Rust).
- **Versionamento:** SemVer. Em `0.x`, a interface de CLI pode mudar; mudanças de
  formato de saída vão para o CHANGELOG.

---

## 15. Segurança e privacidade

- Nenhuma credencial lida, escrita ou solicitada (N3, DD-05).
- Nenhuma telemetria (N7).
- Acesso à rede apenas o que o `git` e o `gh` já fariam, com as credenciais que o
  usuário já configurou.
- `setup` faz backup de `~/.gitconfig` em `~/.gitconfig.forja.bak` antes da
  primeira escrita de cada execução.
- Comandos externos são executados com argumentos em vetor, **nunca via shell** —
  não há interpolação de string em comando, portanto não há vetor de injeção via
  conteúdo do TOML.

---

## 16. Métricas de sucesso

| Métrica | Alvo |
|---|---|
| Passos manuais eliminados por `sync` | ≥ 4 (fetch, checagem, rebase, push) |
| Passos manuais eliminados por `repo new` (Fase 2) | ≥ 5 |
| Comandos para configurar git em máquina nova | 1, contra ~7 manuais |
| Divergências de config entre máquinas | 0, verificável por `setup --dry-run` |
| **Uso real dos fluxos** | `sync` e `cleanup` usados no trabalho diário por 30 dias sem recair nos comandos manuais |
| Incidentes de perda de trabalho causados pela ferramenta | 0, absoluto |
| `panic!` não tratado em uso normal | 0, garantido por lint em CI |
| Custo de adicionar um fluxo novo | Um módulo + uma linha de registro (RNF-07) |

A linha em negrito é a que decide o destino do projeto. Setup declarativo é fácil
de construir e fácil de abandonar; se os fluxos não grudarem no hábito diário, a
tese de §3.1 está errada.

---

## 17. Riscos e questões em aberto

### Riscos

| ID | Risco | Impacto | Prob. | Mitigação |
|---|---|---|---|---|
| R-01 | Scope creep: virar um wrapper genérico de git, com dezenas de subcomandos rasos | Alto | Alta | DD-07 como critério objetivo de admissão (3+ passos ou checagem de segurança). Regra de progressão de fases (§7.3). |
| R-02 | Os fluxos codificados não batem com o fluxo real de trabalho do autor, e a ferramenta é ignorada | Alto | Média | MVP entrega só dois fluxos e exige 2 semanas de uso real antes da Fase 2. `[flow]` permite configurar estratégia e base. Critério de falseamento em §3.1. |
| R-03 | Um fluxo destrói trabalho não publicado | Alto | Baixa | DD-08 (nunca force, nunca stash automático, nunca deletar não-mergeada), testes dedicados por regra (§13), `--dry-run` |
| R-04 | `gh` muda formato de saída e quebra a Fase 2 | Médio | Média | Consumir apenas `gh --json` com campos explícitos, nunca fazer parsing da saída humana. Versão mínima checada pelo `doctor`. |
| R-05 | Dependência externa ausente ou não autenticada | Médio | Média | `doctor` + checagem antes de cada invocação + exit code dedicado (3) |
| R-06 | Schema muda e quebra configs antigas | Baixo | Média | `version` desde o dia 1, política de §8.5, DD-06 |
| R-07 | Sobreposição com `git-extras` e `lazygit` torna a ferramenta redundante | Baixo | Média | O diferencial é a travessia git↔GitHub (§3.1), que nenhum dos dois faz. Reavaliar se a Fase 2 não entregar isso. |

### Questões em aberto

- **QA-01 — Nome.** `forja` é provisório. Validar antes de publicar: livre no
  crates.io; org/repo livre no GitHub; `which forja` vazio em máquina com
  Foundry, Node e Go; sem fórmula homônima no Homebrew; busca `"forja" cli` sem
  produto estabelecido na primeira página.
  *Descartados:* `forge` (colide com o binário `forge` do Foundry, além de
  Minecraft Forge e Laravel Forge), `devforge` (genérico e longo para uso
  diário), `workbench` (colide com MySQL/GNOME Workbench e sugere GUI), `envcli`
  ("env" comunica variáveis de ambiente).
- **QA-02 — `sync` em branch protegida.** Abortar sempre, ou permitir um
  fast-forward puro em `main`? Fast-forward é seguro, mas abre exceção numa
  regra que vale por ser absoluta. Decidir antes de implementar.
- **QA-03 — Detecção da branch base.** Usar `origin/HEAD`, o default do
  repositório via `gh`, ou o `default_branch` da config? Ordem de precedência
  precisa ser definida e documentada.
- **QA-04 — Escopo de `git config`.** Apenas `--global`, ou também `--local`?
  MVP é `--global`. `--local` abre um caso de uso diferente (config por projeto)
  que talvez mereça produto próprio.
- **QA-05 — Confirmação por padrão.** `cleanup` confirma antes de deletar. Os
  fluxos que fazem push deveriam confirmar também, ou isso vira fricção que leva
  o usuário a rodar sempre com `--yes` (anulando a proteção)?

---

## 18. Marcos

| Marco | Conteúdo | Critério de conclusão |
|---|---|---|
| **M0** | Esqueleto: CLI com `clap`, parsing de TOML, `show`, executor de comandos externos | `forja show` exibe config válida; erro claro em config inválida |
| **M1** | MVP: `init`, `doctor`, `setup`, `sync`, `cleanup`, `--dry-run` | Todos os RF do MVP passam; testes de integração com repositórios temporários; todas as regras de DD-08 com teste dedicado; CI verde em Linux e macOS |
| **M1.5** | Duas semanas de uso real + refatoração + release `0.1.0` | RNF-07 satisfeito; binários publicados; README com exemplos reais |
| **M2** | Fase 2: `pr` e `repo new` via `gh` | Repositório criado e PR aberto de ponta a ponta em um comando cada |
| **M3** | Fase 3: perfis, `includeIf`, `capture` | `capture` gera TOML que, reaplicado, produz zero divergências |
| **M4** | Fase 4: `[runtimes]` no `doctor`, config em múltiplos caminhos | — |

---

## 19. Glossário

- **Fluxo:** comando de `forja` que substitui uma sequência de três ou mais
  passos de git/`gh`, com checagens de segurança embutidas.
- **Conformidade:** grau em que o estado real da máquina corresponde ao declarado
  no `forja.toml`.
- **Divergência:** diferença entre o estado atual e o declarado.
- **Idempotente:** executar N vezes produz o mesmo estado final que executar uma.
- **Declarativo:** o arquivo descreve o *estado desejado*, não os *passos* para
  chegar nele.
- **Aborto seguro:** interrupção deliberada de um fluxo antes de qualquer
  alteração, por pré-condição não satisfeita. Sinalizado pelo exit code 4.
