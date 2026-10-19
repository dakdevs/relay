# Compatibility review

Status: review of `provider-compatibility.md` and `routing-architecture.md`.
Research only; no code, no app UI, no credentials or settings touched, nothing
tested against live accounts. Review date: 2026-10-18.

**Verdict.** Both documents are technically sound on wire protocols. Three
corrections were needed and have been applied to `routing-architecture.md`
(§1.1, §2.2, §2.6, §4, §6, §7):

1. t3code integration was presented as working; it is an untested
   configuration hypothesis.
2. The Claude row combined a router key with an expectation of subscription
   use; the official docs say a router key turns the subscription off.
3. "Retry before client bytes" allowed duplicate upstream execution after
   ambiguous transport failures.

`provider-compatibility.md` has since been updated to match §3: its summary,
§1 limitations, §4 verdicts, §6 scope and §7 questions now follow this review.

Labels: **[doc]** official provider documentation or terms, fetched
2026-10-18; **[src]** pinned upstream source; **[c7]** Context7 excerpt;
**[inf]** inference.

## 1. Sources checked independently

| Source                                                                                                                        | Method                                          | Used for                                             |
| ----------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------- |
| [Claude Code: Legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)                                     | Context7 `/websites/code_claude`                | OAuth vs API-key use                                 |
| [Claude Code: Gateways](https://code.claude.com/docs/en/gateways)                                                             | Context7                                        | Which variable turns off the subscription            |
| [Claude Code: Gateway protocol](https://code.claude.com/docs/en/llm-gateway-protocol)                                         | Context7                                        | `anthropic-beta` OAuth capability, custom headers    |
| [Anthropic Consumer Terms](https://www.anthropic.com/legal/consumer-terms)                                                    | WebFetch                                        | Account sharing, automated access                    |
| [OpenAI Codex: Auth](https://learn.chatgpt.com/docs/auth)                                                                     | OpenAI Developer Docs tool                      | `requires_openai_auth`, login caching, access tokens |
| [OpenAI Terms of Use](https://openai.com/policies/row-terms-of-use/), effective 2026-01-01                                    | Headless browser (WebFetch and curl return 403) | Account sharing, rate limits                         |
| `openai/codex` `model-provider/src/auth.rs`, `provider.rs`                                                                    | Context7 `/openai/codex`                        | Auth resolution, workspace-routing gate              |
| t3code `apps/server/src/provider/CodexManagedRuntime.ts`, `docs/user/providers-claude.md`, `docs/user/usage.md` at `9077e818` | GitHub API                                      | Managed vs existing Codex, router-key docs           |

The OpenAI Terms were listed as "not reviewed" in `provider-compatibility.md`
§1; they are reviewed here.

## 2. Reconciled settings

### 2.1 Claude: subscription vs router key

- "Setting `ANTHROPIC_AUTH_TOKEN` for a gateway you run … turns off subscription
  login for that session. … The exception is setting only `ANTHROPIC_BASE_URL`,
  with no gateway credential. … a saved claude.ai login stays the active
  credential, so the subscription's usage limits and billing apply."
  ([Gateways](https://code.claude.com/docs/en/gateways)) [doc]
- In that mode `anthropic-beta` "carries an OAuth capability that the upstream
  requires, and stripping it fails those requests with `401`."
  ([Gateway protocol](https://code.claude.com/docs/en/llm-gateway-protocol))
  [doc]
- The same page says headers set in `ANTHROPIC_CUSTOM_HEADERS` "appear on
  requests as well" [doc]. That gives the router a way to authenticate and
  attribute local clients without a credential variable. That this does not
  disturb the OAuth login is [inf] until tested.
- t3code's OpenRouter recipe (`ANTHROPIC_AUTH_TOKEN` plus empty
  `ANTHROPIC_API_KEY` plus `/logout`) is key mode by construction
  ([providers-claude.md](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/docs/user/providers-claude.md)),
  and t3code warns that such connections may not report subscription limits
  ([usage.md](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/docs/user/usage.md))
  [src].

**Correction:** the original architecture §1.1 used the OpenRouter recipe for
every Claude instance. Using it would silently replace subscriptions with
API-key billing. The architecture now lists the two modes separately.

### 2.2 Codex: T3 managed vs existing

- Managed mode builds a fixed provider `openai_token_sharing` with
  `base_url="https://api.openai.com/v1"` and `env_key="ACCESS_TOKEN"`, sets
  `ACCESS_TOKEN` to the T3-owned token, and deletes `T3CODE_CODEX_LAUNCH_ARGS`,
  `OPENAI_API_KEY` and `OPENAI_BASE_URL`, with the comment "Ambient CLI
  overrides cannot redirect a T3-owned token to a different provider"
  ([CodexManagedRuntime.ts](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/apps/server/src/provider/CodexManagedRuntime.ts))
  [src]. A router cannot sit in front of a managed instance.
- Routing requires `setupMode: "existing"` with the user's own `CODEX_HOME`.
- Subscription mode: "Set `requires_openai_auth = true` … You can then sign in
  with ChatGPT or an API key. This is useful when you access OpenAI models
  through an LLM proxy server. When `requires_openai_auth = true`, Codex
  ignores `env_key`." ([Auth](https://learn.chatgpt.com/docs/auth)) [doc]
- Workspace routing (account UI, model list, cloud tasks) requires the first-party
  auth path **and** a base URL that is unset or ends in `/backend-api/codex`
  (`provider.rs`) [c7]. A `/v1` router URL skips it; the effect on t3code's
  account display is [inf].

**Correction:** the original architecture §1.1 said its `-c` overrides were "the same
mechanism t3code's `CodexManagedRuntime.ts` uses", implying managed instances
could be pointed at the router. They cannot. It also showed only
`requires_openai_auth=false` with `env_key`, which is key mode.

## 3. Authorization: what the evidence does and does not say

Personal use of one's own accounts and routing on behalf of others are
different questions. The prior document's verdict column treats missing
permission as "Blocked". The table below separates explicit text from
inference.

| Question                                                                              | Explicit text                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Reading                                                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude: own subscription login passing through a local gateway                        | Gateway docs describe the mode [doc]. The legal page does not "prevent an end user from signing in to the unmodified Claude Code binary with their own Claude subscription" [doc]                                                                                                                                                                                                                                                                                                                                 | **Supported** by documentation                                                                                                                                                                                                                                                                                               |
| Claude: routing plan credentials for other people                                     | "Anthropic does not permit third-party developers … to route requests through Free, Pro, or Max plan credentials on behalf of their users" [doc]. Consumer Terms: "You may not share your Account login information … You also may not make your Account available to anyone else" [doc]                                                                                                                                                                                                                          | **Prohibited**                                                                                                                                                                                                                                                                                                               |
| Claude: one person's router holding tokens for several of their **own** subscriptions | Legal page: OAuth is "intended exclusively for purchasers of … subscription plans and is designed to support ordinary use of Claude Code and other native Anthropic applications". In the bullet on "Developers building products or services": "developers may not collect, store, or intermediate Claude.ai credentials or session tokens" [doc]. Consumer Terms forbid automated access "except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it" [doc] | **Unresolved.** The intermediation clause is addressed to developers of products and services; whether a personal tool is covered is not stated. Requests still originate from genuine Claude Code, which Anthropic does permit. Enforcement "may [happen] without prior notice" [doc]. Not proven prohibited, not permitted |
| Claude: disguising a non-Claude-Code client as Claude Code                            | No clause names cloaking. Consumer Terms forbid "bypassing any of our systems or protective measures" [doc]                                                                                                                                                                                                                                                                                                                                                                                                       | **Likely conflicts** [inf]; also unnecessary, because t3code runs the genuine CLI                                                                                                                                                                                                                                            |
| Codex: own ChatGPT login through a local proxy                                        | Auth docs, quoted in §2.2 [doc]. No upstream is named                                                                                                                                                                                                                                                                                                                                                                                                                                                             | **Documented as useful; upstream unspecified**                                                                                                                                                                                                                                                                               |
| Codex: copying one's own login to another machine                                     | Auth docs give step-by-step instructions to copy `~/.codex/auth.json` to a headless machine or container [doc]                                                                                                                                                                                                                                                                                                                                                                                                    | **Documented** for own use. This weakens the reading that ChatGPT tokens may never leave the stock CLI's home                                                                                                                                                                                                                |
| Codex: sharing with others                                                            | OpenAI Terms: "You may not share your account credentials or make your account available to anyone else" [doc]                                                                                                                                                                                                                                                                                                                                                                                                    | **Prohibited**                                                                                                                                                                                                                                                                                                               |
| Codex: router pooling several of one's own ChatGPT accounts                           | OpenAI Terms forbid "circumvent any rate limits or restrictions or bypass any protective measures" and "Automatically or programmatically extract data or Output" [doc]. No clause addresses multiple own accounts                                                                                                                                                                                                                                                                                                | **Unresolved.** Whether binding sessions across one's own accounts "circumvents" per-account limits is a judgement OpenAI has not published                                                                                                                                                                                  |

Corrections applied to `provider-compatibility.md`:

- §4, ChatGPT OAuth pooling row: the quote "trusted scripts, schedulers, and
  private CI runners" refers to **ChatGPT Enterprise Codex access tokens**
  ([Auth](https://learn.chatgpt.com/docs/auth),
  [Access tokens](https://learn.chatgpt.com/docs/enterprise/access-tokens)),
  not to ordinary ChatGPT-login tokens. It is not evidence about pooling.
- §4, "Not authorized by any evidence found. Blocked": absence of permission is
  not prohibition. Correct reading: own-account pooling is unresolved;
  pooling for others is prohibited by the Terms quoted above.
- §4, Claude pooling row "Blocked": the quoted clauses support "prohibited when
  routing for other users". For a single owner they are ambiguous, as above.
- §6 "Blocked, out of v1": "Running OAuth flows or storing subscription
  tokens" is a scope choice for this task, not a provider rule. Cloaking and
  UA/TLS spoofing remain excluded on the risk reading above and because they
  are not needed.
- §1 Limitations: the OpenAI Terms of Use are now reviewed.

## 4. Safe retry

`provider-compatibility.md` §5 is correct that client-side commit is necessary
but not sufficient. The original architecture §2.2 was wrong on three points:

- It retried "transport errors" and 500/502/504 before response headers. After
  the body is written, those are ambiguous: the provider may have executed and
  billed the request.
- It stated "a retried request never ran a tool". Server-side tools (Anthropic
  web search, web fetch, code execution; Responses hosted tools) run upstream
  before any byte returns.
- It stated each request "is billed at most once on the original account". Each
  attempt that reached the upstream may be billed on its own account.

The architecture now classifies each attempt as proven-not-sent, explicit
rejection, ambiguous, or committed. It retries across accounts only when the
attempt was proven not sent or explicitly rejected, the request is
stateless-eligible, and the router owns account choice. No idempotency key for
Messages or Responses appeared in the sources reviewed; this is unverified, not
a confirmed absence.

## 5. Scope preserved

Subscription functionality is kept, not replaced by API keys:

- **Mode A** (pass-through, instance chooses account, router observes and
  recommends) is the v1 default. Quota-aware recommendation and token
  analytics work here.
- **Mode B** (own-account pool, router binds sessions by headroom) is designed
  and opt-in. It meets the full "quota-aware routing across my subscriptions"
  request; its terms position is unresolved, as §3 states.
- **Mode D** (API keys) is offered alongside, never as a silent substitute.

## 6. Open items

1. User decision: enable Mode B after reading §3.
2. Live checks, all pending: a Claude subscription-mode turn with
   `ANTHROPIC_CUSTOM_HEADERS`; a Codex `existing` turn with
   `requires_openai_auth=true` and the upstream it needs; t3code limits and
   account UI in each mode.
3. Mode B token ownership: whether router-side refresh rotates the CLI's
   refresh token [inf].

## 7. Validation gates

This workspace has no `package.json`, git repository or project scripts.
`bun test`, `bun run test:e2e` and a lint script therefore do not exist and
were **not run**. The only check that applies is Prettier on the edited
Markdown files.
