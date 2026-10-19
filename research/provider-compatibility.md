# Provider compatibility and authorization research

Status: research only. Nothing is implemented. No accounts were connected and no
user credentials or settings were read or changed. Research date: 2026-10-18.

**Bottom line.** A router can likely be wired into t3code for Claude and Codex
with no t3code changes, but no instance has completed a turn through a router
yet. Routing several Claude or ChatGPT _subscriptions_ is technically proven by
CLIProxyAPI. The terms evidence splits three ways:

- **Prohibited:** routing subscription credentials on behalf of other users, or
  sharing an account's login. Anthropic and OpenAI both say so explicitly (§4).
- **Supported:** the user's own login passing through a local gateway from the
  stock CLI (Claude: documented; Codex: documented as "useful", upstream
  unspecified).
- **Unresolved:** one person's router holding tokens for several of their
  **own** subscriptions and choosing an account per new session. No clause
  found permits it. The closest Anthropic clause is addressed to developers of
  products and services. OpenAI's ban on circumventing rate limits doesn't say
  whether it covers this. Absence of permission is not treated as prohibition.

§6 keeps the requested quota-aware subscription routing as an opt-in mode,
alongside pass-through and API-key modes. It is not replaced by API keys or by
observer-only scope. See `compatibility-review.md` for the independent review.

Evidence labels used below:

- **[src]**: read in upstream source at the pinned revision.
- **[doc]**: official provider documentation, fetched 2026-10-18.
- **[c7]**: Context7 excerpt of upstream source. It was not checked against a
  pinned revision.
- **[inf]**: inference that has not been verified.

## 1. Sources and revisions

| Source                                                                          | Revision                                   | Commit date               |
| ------------------------------------------------------------------------------- | ------------------------------------------ | ------------------------- |
| [router-for-me/CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) (MIT) | `2422c6b0f5fe16495320fa29456493874c1977a0` | 2026-10-18T21:05:44+08:00 |
| [pingdotgg/t3code](https://github.com/pingdotgg/t3code) (MIT)                   | `9077e818d836ec6e9cc373d7469034cccc593a5a` | 2026-10-18T18:27:47-07:00 |

These were shallow clones of the default branch. In the links below,
`CPA = https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0`
and `T3 = https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a`.

Official documents:

- Claude Code, [Legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)
- Claude Code, [Other LLM gateways](https://code.claude.com/docs/en/llm-gateway)
- Claude Code, [Gateway compatibility guide](https://code.claude.com/docs/en/llm-gateway-protocol)
- OpenAI Codex, [Auth](https://learn.chatgpt.com/docs/auth) (redirected from developers.openai.com/codex/auth)
- OpenAI Codex, [Pricing](https://learn.chatgpt.com/docs/pricing)
- OpenAI, [Terms of Use](https://openai.com/policies/row-terms-of-use/) (effective 2026-01-01)
- Anthropic, [Consumer Terms](https://www.anthropic.com/legal/consumer-terms)

Context7 was used for `/anthropics/claude-code` and `/openai/codex`.

Limitations:

- The OpenAI Terms of Use returned HTTP 403 to WebFetch and curl. A later
  review read them in a headless browser:
  [Terms of Use](https://openai.com/policies/row-terms-of-use/), published and
  effective 2026-01-01 (`compatibility-review.md` §1, §3).
- Anthropic's [Consumer Terms](https://www.anthropic.com/legal/consumer-terms)
  were fetched in the later review and are quoted in §4.
- Nothing was tested against live accounts.

## 2. How t3code reaches providers

t3code does not call model APIs directly. Each provider _instance_ spawns a
stock agent CLI or SDK, and that program makes the network calls.

| Driver                                                                  | Process                            | Per-instance knobs a router can use                                                                                                                                                                                                                                        |
| ----------------------------------------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `claudeAgent`                                                           | Claude Agent SDK → `claude` binary | `binaryPath`, `homePath` (`CLAUDE_CONFIG_DIR`), `launchArgs`, `environment[]` ([T3/packages/contracts/src/settings.ts#L615-L668](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/packages/contracts/src/settings.ts#L615-L668)) [src]    |
| `codex`                                                                 | `codex app-server` (JSON-RPC)      | `setupMode` `managed`\|`existing`, `binaryPath`, `homePath` (`CODEX_HOME`), `shadowHomePath`, `launchArgs` ([settings.ts#L556-L610](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/packages/contracts/src/settings.ts#L556-L610)) [src] |
| `cursor`, `grok`, `opencode`, `pi`, `muse`, `antigravity`, ACP registry | Each vendor's CLI or ACP agent     | Mostly API-key environment variables (`CURSOR_API_KEY`, `XAI_API_KEY`, `META_API_KEY`); no common base-URL setting found [src]                                                                                                                                             |

The `environment` field is a generic per-instance list of
`{name, value, sensitive}` entries. It is merged over the server's process
environment when the CLI is spawned
([providerInstance.ts#L38-L61](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/packages/contracts/src/providerInstance.ts#L38-L61),
[instanceEnvironment.ts](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/packages/provider-core/src/server/instanceEnvironment.ts))
[src]. Several instances of the same driver, each with its own home directory,
are how t3code already supports multiple subscriptions: one account per
instance.

**Managed Codex cannot be redirected.** `CodexManagedRuntime` pins
`base_url="https://api.openai.com/v1"`, injects the T3-owned token, and deletes
`OPENAI_BASE_URL`, `OPENAI_API_KEY` and `T3CODE_CODEX_LAUNCH_ARGS`. Its comment
reads "Ambient CLI overrides cannot redirect a T3-owned token to a different
provider"
([CodexManagedRuntime.ts#L18-L31, #L93-L100](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/apps/server/src/provider/CodexManagedRuntime.ts#L18-L100))
[src]. Router integration therefore requires `setupMode: "existing"`. In that
mode `launchArgs` reach `codex app-server`, unless `T3CODE_CODEX_LAUNCH_ARGS`
overrides them
([codexLaunchArgs.ts#L5-L15](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/apps/server/src/provider/codexLaunchArgs.ts#L5-L15))
[src].

### 2.1 Existing quota and usage surfaces in t3code

- **Token analytics** come from local CLI history and SDK result events. One
  example is `ClaudeTurnTokenUsage.ts`, which sums `input_tokens`,
  `cache_read_input_tokens`, `cache_creation_input_tokens` and `output_tokens`.
  For Codex, the source is the app-server's `thread/tokenUsage/updated`
  notification. This history is already broken down per instance, and so per
  account [src].
- **Quota windows** use the schema `ServerProviderUsageWindow{id, kind: session|weekly|monthly|other, usedPercent, resetsAt, windowDurationMins}`.
  The data comes from:
  - Claude's `rate_limit_event`;
  - Codex's `account/rateLimits/updated` notification, a sparse merge by `id`;
  - probes of `https://api.anthropic.com/api/oauth/usage` and
    `https://chatgpt.com/backend-api/wham/usage`.

  See [providerUsageLimits.ts#L15-L68](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/packages/contracts/src/providerUsageLimits.ts#L15-L68) [src].

- **t3code already integrates CLIProxyAPI as a usage hub.** It uses
  `usageLimitSources[*] = {kind: "cliproxy", url, managementKey}`
  ([settings.ts#L750-L757](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/packages/contracts/src/settings.ts#L750-L757))
  [src]. The client
  ([apps/server/src/usage/cliproxyApi.ts](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/apps/server/src/usage/cliproxyApi.ts))
  calls three endpoints:
  - `GET /v0/management/auth-files` (L195);
  - `POST /v0/management/api-call` with `{auth_index, method, url, header: {Authorization: "Bearer $TOKEN$", ...}}` (L217). The hub substitutes the stored token and fetches the provider usage URL (L265 Claude, L295 Codex);
  - `POST /v0/management/reset-quota` (L397).

  The t3code user docs say this hub "supplies usage information; configure the
  provider separately to send agent requests through the hub"
  ([docs/user/usage.md#L124-L134](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/docs/user/usage.md#L124-L134))
  [src].

- **A proxy credential removes Claude subscription limits from t3code's
  display.** The docs say: "API-key accounts may not report subscription limits.
  This also applies to Claude connections using a proxy through
  `ANTHROPIC_AUTH_TOKEN`" ([usage.md#L121-L122](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/docs/user/usage.md#L121-L122))
  [src].

**Implication: do not clone the hub API.** `api-call` is a generic primitive
that sends a request with a stored token to any URL the caller names. Copying it
would require the router to store subscription tokens (§4) and would create a
credential-forwarding endpoint. The router should publish its own
read-only snapshots and leave t3code's hub integration alone.

## 3. Wire protocols and endpoint overrides

### 3.1 Claude Code → Anthropic Messages [doc]

- **Endpoint override:** `ANTHROPIC_BASE_URL`. The CLI then sends:
  - `POST /v1/messages?beta=true` (SSE);
  - optionally `POST /v1/messages/count_tokens`;
  - `HEAD /api/hello` as a warm-up, which may be rejected;
  - `GET /v1/models?limit=1000`, only when
    `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1`. It has a 3 s timeout, and
    any redirect counts as failure.
- **Credential variables:**
  - `ANTHROPIC_AUTH_TOKEN` is sent as `Authorization: Bearer`.
  - `ANTHROPIC_API_KEY` is sent as `x-api-key`.
  - `apiKeyHelper` supplies a rotating credential.
  - **If any of these is active, the claude.ai login is not sent** and the
    subscription's limits do not apply.
  - **If only `ANTHROPIC_BASE_URL` is set, the saved claude.ai login stays the
    credential.** In that case the gateway "must forward the OAuth capability in
    `anthropic-beta`"; stripping it causes `401`.
- **Must forward unchanged:**
  - `anthropic-version` and `anthropic-beta`, verbatim and never allowlisted.
  - All `anthropic-*` headers and all body fields, treated as open lists.
  - The `system` array, unchanged and in order. Its first block is an
    attribution block that `api.anthropic.com` strips by position.
  - `cache_control` markers.
  - Error bodies, unmodified, because the client's capability-rejection retry
    matches on their wording.
- **Streaming requirements:**
  - Relay events as they arrive. Buffering stalls the client.
  - Deliver the full sequence through `message_delta` and `message_stop`.
    Early EOF is treated as a dropped connection.
  - Keep `ping` events, or the idle watchdog fires during long thinking.
- **Response headers the CLI reads:**
  - `retry-after` in integer seconds; a value above 60 stops the client's own
    retries.
  - `x-should-retry`.
  - `anthropic-ratelimit-unified-*`, used for plan-limit display and to tell a
    429 plan limit from a throttle.
- **Attribution headers the router can use:**
  - `x-claude-code-session-id`, `x-claude-code-agent-id` and
    `x-claude-code-parent-agent-id`.
  - Opt-in hints via `CLAUDE_CODE_GATEWAY_HINT_HEADERS=1`:
    `x-claude-code-request-class` (main, subagent, workflow, compaction,
    auxiliary), `x-claude-code-prompt-id`, `x-claude-code-compaction`, and
    others.
- **Gaps:**
  - Fast-mode availability and the WebFetch safety check call
    `api.anthropic.com` directly, bypassing the router.
  - Under the HIPAA configuration, gateway use is not eligible.

**Cross-check against `routing-architecture.md` §1.1, Claude row.** The
documented mechanics are correct. t3code documents the same pattern for
OpenRouter, including `ANTHROPIC_API_KEY` explicitly empty and `/logout` in that
config directory
([providers-claude.md#L79-L93](https://github.com/pingdotgg/t3code/blob/9077e818d836ec6e9cc373d7469034cccc593a5a/docs/user/providers-claude.md#L79-L93)).

**But** `ANTHROPIC_AUTH_TOKEN=<router key>` replaces the subscription login. The
router would then need its own upstream credential:

- With an **Anthropic Console API key**, this is the documented gateway use
  case.
- With **Claude subscription OAuth tokens**, its terms position depends on whose accounts they are (§4).

For subscriptions, the only documented configuration is **`ANTHROPIC_BASE_URL`
alone**: the per-instance login passes through and the account stays fixed. Two
t3code limitations follow:

- In router-key mode, t3code stops showing subscription limits for that
  instance (§2.1).
- t3code's own `/api/oauth/usage` probe runs against the instance's login, not
  the router.

### 3.2 Codex → OpenAI Responses [src, c7, doc]

**Wire API:** `POST {base_url}/responses` as SSE, or as WebSocket when the
provider sets `supports_websockets=true`. Codex also uses
`POST {base_url}/responses/compact` for remote compaction, which returns an
opaque `encrypted_content`. Codex's ChatGPT backend is
`https://chatgpt.com/backend-api/codex/responses`
([CPA codex_websockets_stream.go#L773](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/internal/runtime/executor/codex_websockets_stream.go#L773)).
That is a ChatGPT-internal surface. OpenAI does not document it as a public API.

**Endpoint overrides** [c7]:

- **`openai_base_url`** overrides the built-in OpenAI provider.
- **`chatgpt_base_url`** relocates the ChatGPT backend. Admins can enforce it.
- **`model_providers.<id>`** defines a custom provider with these fields:
  `base_url`, `env_key`, `experimental_bearer_token`, `wire_api="responses"`,
  `requires_openai_auth`, `supports_websockets`, `http_headers` and
  `query_params`.
- Project-local `config.toml` cannot set any of these. A `-c` override from
  t3code's `launchArgs` sits in the runtime layer, so it is allowed.

**Auth resolution order for a custom provider** [c7]:

1. `env_key` → bearer token.
2. `experimental_bearer_token`.
3. If `requires_openai_auth=true` and neither of the above is set → the ChatGPT
   or API-key login from `auth.json`. The bearer provider then also sends
   `ChatGPT-Account-ID`.

The Auth docs say `requires_openai_auth = true` "is useful when you access
OpenAI models through an LLM proxy server" [doc].

**Codex-backend features gate on the base URL** [c7]. ChatGPT workspace routing
covers the account UI, the model list, cloud tasks and memories. It runs only
when the provider uses the first-party auth path **and**
`supports_codex_backend_routes()` holds, meaning the base URL is unset or ends
in `/backend-api/codex`. A router base URL such as
`http://127.0.0.1:P/v1` skips workspace routing. [inf] t3code's Codex account
and plan display may therefore degrade. Exposing `/backend-api/codex` on the
router, as CLIProxyAPI does at
[server_routes.go#L118-L127](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/internal/api/server_routes.go#L118-L127),
might keep the gate open. Whether the check also requires an "OpenAI"
provider ID is unverified.

**Quota data from Codex:**

- **Response headers:**
  - `x-codex-primary-*` and `x-codex-secondary-*` (`used-percent`,
    `window-minutes`, `reset-at`, `reset-after-seconds`);
  - `x-codex-plan-type`, `x-codex-credits-*` and `x-codex-active-limit`;
  - per-limit namespaces such as `x-codex-<name>-primary-*` on HTTP and
    `x-codex-additional-<limit>-*` on WebSocket.

  These names come from CLIProxyAPI's
  [quota_signals.go#L150-L200](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/sdk/cliproxy/auth/quota_signals.go#L150-L200)
  [src]. OpenAI does not document them.

- **In-stream:**
  - `TokenCountEvent{info, rate_limits}` becomes `thread/tokenUsage/updated`
    plus `account/rateLimits/updated` [c7].
  - Quota exhaustion arrives as `error.type == "usage_limit_reached"` with
    `resets_in_seconds`, sometimes inside an HTTP 200 stream
    ([CPA codex_executor_terminal.go#L403-L440](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/internal/runtime/executor/codex_executor_terminal.go#L403-L440))
    [src].

**Cross-check against `routing-architecture.md` §1.1, Codex row.**

- The `-c` mechanism and the field names are correct, but they require
  `setupMode: "existing"` (§2).
- `env_key="ROUTER_KEY"` with `requires_openai_auth=false` means Codex sends
  only the router key. The router must then hold an upstream credential:
  - an **OpenAI Platform API key** to `api.openai.com/v1/responses` is
    documented and authorized;
  - **ChatGPT OAuth tokens** to `chatgpt.com/backend-api/codex` are
    undocumented as a public API; terms position depends on whose accounts they are (§4).
- For a subscription pass-through, the closest documented path is
  `requires_openai_auth=true` with no `env_key`. The instance's own ChatGPT
  login then reaches the router, which forwards it unchanged.
  - [inf] The forward target would be `chatgpt.com/backend-api/codex`, still an
    undocumented surface. The docs say only that the setting "is useful" with a
    proxy; they do not say which upstream the proxy may use.
  - Treat this path as **technically plausible, authorization unclear**.
- The note "`supports_websockets=false` keeps v1 to HTTP+SSE" is correct for
  custom providers.
- The architecture's claim that "the router does not read the CLIs' own
  credentials" is true **only** in key mode. In pass-through mode, credentials
  transit the router in memory. The router must not log or persist them.

### 3.3 CLIProxyAPI's approach (reference, not a template) [src]

**Ingress routes** ([server_routes.go#L64-L127](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/internal/api/server_routes.go#L64-L127)):

- `/v1/chat/completions`, `/v1/responses` (POST, plus GET as WebSocket),
  `/v1/responses/compact`
- `/v1/messages`, `/v1/messages/count_tokens`, `/v1/models`
- `/backend-api/codex/*`, `/v1beta/*` for Gemini, and realtime routes

The proxy translates any of these formats into any upstream.

**Account isolation:**

- Each upstream account is a JSON auth file in `oauth.auth-dir`, default
  `~/.cli-proxy-api`
  ([config.example.yaml#L914](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/config.example.yaml#L914)).
- The proxy runs its **own** OAuth flows: Claude via `api.anthropic.com/api/oauth/*`,
  Codex via `auth.openai.com/oauth/*` and device auth.
- Downstream clients authenticate with proxy API keys only.

**Client impersonation** (relevant to §4):

- **Claude.** "Request cloaking" disguises non-Claude-Code clients as the
  Claude Code CLI. It is on by default for unrecognized clients, through
  `disable-claude-cloak-mode: false` and mode `auto`
  ([config.example.yaml#L430-L435, #L722-L729](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/config.example.yaml#L430-L435);
  `claude_executor_cloaking.go`; `X-App: cli`, Claude Code beta baseline in
  `claude_executor_request.go#L135, #L1478`).
- **Codex.** The proxy sends `Originator: codex-tui` and the hardcoded
  User-Agent `codex-tui/0.154.0 (Mac OS 26.5.2; arm64) iTerm.app/3.6.11`
  ([codex_executor_request.go#L28-L29](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/internal/runtime/executor/codex_executor_request.go#L28-L29)).
  It uses a uTLS "Chrome ChatGPT/Codex transport" (config comment under
  `requests.tls-handshake-timeout-seconds`).

**Routing and quota:**

- Strategies: `round-robin`, `weighted-round-robin` and `fill-first`.
- `session-affinity` is **off by default**
  ([config.example.yaml#L162](https://github.com/router-for-me/CLIProxyAPI/blob/2422c6b0f5fe16495320fa29456493874c1977a0/config.example.yaml#L162)).
  Its stated purpose is "Prompt/KV Cache reuse". Subagents can inherit the
  parent's credential.
- Quota is observed passively, for `claude`, `codex` and `devin` only.
- `streaming.bootstrap-retries` defaults to 0.
- Since v6.10.0, the built-in usage statistics were removed and moved to
  external tools.

The architecture doc's §1.2 summary matches the source.

## 4. Authorization evidence (separate from technical compatibility)

| Integration                                                                                                                                                  | Technically works   | Official authorization evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | Verdict                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude Code + router + **Anthropic Console API key** held by router                                                                                          | Yes [doc]           | Gateway docs describe this exactly. Legal page: customers may provision their own API keys "provided the resulting usage is billed to the key owner" and is not resold [doc]                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | **Authorized** for the key owner's own use                                                                                                                                                                                                                                                                                 |
| Claude Code + router (`ANTHROPIC_BASE_URL` only), **user's own subscription login passes through** unchanged, one account per t3code instance                | Yes [doc]           | Gateway docs explicitly describe this mode and require forwarding the OAuth beta [doc]. Legal page does not prevent "an end user from signing in to the unmodified Claude Code binary with their own Claude subscription" [doc]. Pro/Max limits "assume ordinary, individual usage" [doc]                                                                                                                                                                                                                                                                                                                                                                                                                                                            | **Documented; closest to sanctioned.** The router must not store, alter, or swap the token                                                                                                                                                                                                                                 |
| Router holds tokens for subscriptions and routes requests **for other users**                                                                                | Yes [src]           | Legal page: "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their users." Consumer Terms §2: "You may not share your Account login information … You also may not make your Account available to anyone else." [doc]                                                                                                                                                                                                                                                                                                                                                                                            | **Prohibited**                                                                                                                                                                                                                                                                                                             |
| One person's router holds OAuth tokens for several of their **own** Claude subscriptions and binds each new session to one (CLIProxyAPI model, single owner) | Yes [src]           | Legal page: OAuth "is intended exclusively for purchasers of Claude Free, Pro, Max, Team, and Enterprise subscription plans and is designed to support ordinary use of Claude Code and other native Anthropic applications." In the bullet on "Developers building products or services": "developers may not collect, store, or intermediate Claude.ai credentials or session tokens — sign-in to a Claude account must complete through Anthropic's own flow." Consumer Terms §3 forbid automated access "Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it". Anthropic "reserves the right to take measures to enforce these restrictions and may do so without prior notice." [doc] | **Unresolved.** The intermediation clause is addressed to developers of products and services; whether a personal tool run by the account owner falls under it is not stated. Requests still come from genuine Claude Code. Not permitted explicitly, not shown to be prohibited; enforcement risk is the user's to accept |
| Any client disguised as Claude Code ("cloaking")                                                                                                             | Yes [src]           | No clause names cloaking. Consumer Terms forbid "bypassing any of our systems or protective measures" [doc]; the disguise exists to evade client detection [inf]                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | **Likely conflicts** [inf]. Unnecessary: t3code runs the genuine CLI                                                                                                                                                                                                                                                       |
| Codex + router + **OpenAI Platform API key**                                                                                                                 | Yes [c7]            | Auth docs: API-key use is billed "at standard API rates"; "API keys are still the recommended default" for automation [doc]                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | **Authorized**                                                                                                                                                                                                                                                                                                             |
| Codex `requires_openai_auth=true` + router as LLM proxy, user's own ChatGPT login passes through                                                             | Plausible [c7, inf] | Auth docs call this "useful when you access OpenAI models through an LLM proxy server" [doc]. No document names `chatgpt.com/backend-api/codex` as a permitted proxy upstream. Auth docs also give steps to copy one's own `~/.codex/auth.json` to another machine [doc]                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | **Documented as useful; upstream unspecified**                                                                                                                                                                                                                                                                             |
| Router lets **other people** use a ChatGPT account                                                                                                           | Yes [src]           | Terms of Use, Registration: "You may not share your account credentials or make your account available to anyone else" [doc]                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | **Prohibited**                                                                                                                                                                                                                                                                                                             |
| One person's router holds ChatGPT tokens for several of their **own** accounts, calls `chatgpt.com/backend-api/codex`, and binds each new session to one     | Yes [src]           | Terms of Use, "What you cannot do": no "Interfere with or disrupt our Services, including circumvent any rate limits or restrictions or bypass any protective measures", and no "Automatically or programmatically extract data or Output" [doc]. No clause addresses several own accounts. The "trusted scripts, schedulers, and private CI runners" wording in the Auth docs concerns ChatGPT Enterprise **Codex access tokens**, not ordinary logins [doc]                                                                                                                                                                                                                                                                                        | **Unresolved.** Whether binding sessions across one's own accounts "circumvents" per-account limits is not published                                                                                                                                                                                                       |
| Spoofing `codex-tui` User-Agent, `Originator` or TLS fingerprint (CLIProxyAPI)                                                                               | Yes [src]           | Terms forbid bypassing "protective measures" [doc]; spoofing indicates the surface is not intended for third-party clients [inf]                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | **Likely conflicts** [inf]. Unnecessary when the genuine Codex CLI sends the request                                                                                                                                                                                                                                       |
| Polling `api.anthropic.com/api/oauth/usage` or `chatgpt.com/backend-api/wham/usage` with stored tokens                                                       | Yes [src]           | Undocumented internal endpoints. t3code calls them only with each instance's own login, or through a hub the user configures                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | Router: **out of v1 scope** (needs stored tokens; endpoints undocumented). Leave this to t3code                                                                                                                                                                                                                            |

This document does not claim any subscription-pooling configuration is
"kosher", nor that own-account pooling is prohibited. Proceeding with an
unresolved row is the user's decision, made against the provider terms, and it
risks account enforcement. Prohibited rows stay out of scope.

## 5. Safe retry and failover boundaries

"Retry only before the first byte reaches the client" is **necessary but not
sufficient**. The client-side tool argument in architecture §2.2 also
overstates safety, for these reasons:

1. **The upstream may have run without the router seeing a response.** If a
   connection resets or times out after the request body was fully written, the
   result is ambiguous. The provider may have generated output, billed it, and
   counted it against quota. A retry on any account can then double-bill or
   double-count. Only failures that prove the request never reached the provider
   are unambiguous:
   - DNS failure, TCP connect failure or TLS handshake failure;
   - HTTP/2 `REFUSED_STREAM` or GOAWAY with a last-stream ID below this
     stream's ID;
   - a 4xx or 5xx status received before any output.
2. **Server-side tools run upstream.** These are actions the model's request
   itself triggers on the provider side:
   - Anthropic: web search and fetch, code execution and containers, the
     advisor tool;
   - Responses: hosted `web_search` and similar tools.

   Once the upstream accepted the request, those actions may have run. They are
   external side effects, and a retry repeats them.

3. **Continuation state is bound to an account or organization:**
   - Responses `previous_response_id` and stored responses;
   - `reasoning.encrypted_content` items;
   - `/responses/compact` `encrypted_content` capsules;
   - Anthropic thinking signatures. The API rejects some of these with "bound to
     a different conversation"
     ([gateway guide](https://code.claude.com/docs/en/llm-gateway-protocol));
     cross-account behaviour is [inf];
   - Anthropic code-execution `container` IDs;
   - prompt caches, which are scoped to an organization. Failing over loses the
     cache and raises cost.

   Moving a session to another account mid-session can fail with a 400, or
   silently drop reasoning context. CLIProxyAPI's session affinity exists for
   this reason.

### 5.1 Proposed boundaries

| Condition                                                                                                                       | Same account                                                                                                             | Different account                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| Proven not-sent (connect, DNS or TLS failure; `REFUSED_STREAM`; GOAWAY below the stream ID)                                     | Retry                                                                                                                    | Retry, only for **stateless-eligible** requests (defined below)                                                              |
| HTTP 429 or 529 with `retry-after`, `anthropic-ratelimit-unified-*` rejection, or Codex `usage_limit_reached` before any output | Do not retry. Mark cooldown                                                                                              | Retry only if stateless-eligible and not pass-through; otherwise return the error unchanged so the CLI's own backoff applies |
| HTTP 5xx, 408 or 502–504 before any output                                                                                      | Retry once after `retry-after`, or after backoff                                                                         | Same as the 429 row                                                                                                          |
| Ambiguous: reset or timeout after the body was sent, or EOF before the final `message_stop` / `response.completed`              | **Never**                                                                                                                | **Never.** Return a retryable error to the client and record `outcome=ambiguous`; the client decides                         |
| In-stream error inside the bootstrap window, before any `content_block_start` / output item                                     | Hold-and-fail-over is allowed only if no server-tool event or output item was seen and the request is stateless-eligible | Same                                                                                                                         |
| Any byte forwarded to the client                                                                                                | Never                                                                                                                    | Never                                                                                                                        |
| 400, 404, 413, context-length, or capability rejection                                                                          | Never. Forward the body unmodified (the CLI matches on its wording)                                                      | Never                                                                                                                        |
| 401 or 403                                                                                                                      | Key mode: one single-flight refresh. Pass-through: never (the router does not own the token)                             | Never                                                                                                                        |

**Stateless-eligible** means all of the following hold:

- There is no `previous_response_id`.
- There is no `encrypted_content` or reasoning item.
- There is no compaction capsule.
- There is no thinking block with a signature.
- There is no `container`.
- The tool list declares no server-side tools.
- The session has no sticky binding, or its bound account is permanently
  unavailable. In that case, return an error rather than migrating silently.

In practice, most turns after the first in a Claude Code or Codex session are
**not** stateless-eligible. Cross-account failover therefore applies mainly to
first turns and to auxiliary requests (titles, classifiers). **Quota-aware
routing must act at session start (bind), not per request.**

## 6. Proposed MVP scope

Mode letters match `routing-architecture.md` §2.6.

**v1 modes:**

1. **Mode A, subscription pass-through (default).**
   - Claude: `ANTHROPIC_BASE_URL` only; router auth and instance attribution
     via `ANTHROPIC_CUSTOM_HEADERS`. Codex: `setupMode: existing`,
     `requires_openai_auth=true`, `http_headers` for attribution.
   - Each t3code instance keeps its own login. The router forwards credentials
     untouched and never persists or logs them.
   - The router records token analytics and quota from response headers and
     recommends the instance with most headroom. The user picks the instance.
2. **Mode B, own-account quota-aware pool (opt-in).** This is the requested
   multi-subscription routing.
   - The router holds OAuth tokens for several of the user's **own** accounts
     and binds each new session (keyed on `x-claude-code-session-id` or the
     Codex session) to the account with most headroom.
   - Localhost only, single user, genuine CLI headers kept, no cloaking or
     UA/TLS spoofing.
   - Terms position: **unresolved** (§4). Enabled only after the user accepts
     that risk. Token acquisition is outside this research task.
3. **Mode D, API-key routing.**
   - Claude: `ANTHROPIC_AUTH_TOKEN=<router key>`, `ANTHROPIC_API_KEY=` (empty).
     Codex: `env_key="ROUTER_KEY"`, `requires_openai_auth=false`.
   - The router holds Anthropic Console or OpenAI Platform keys and routes by
     quota at session start. Offered alongside A and B, never as a silent
     substitute.
4. **Analytics:** per-account and per-model token sums from
   `message_delta` / `message_start` usage and `response.completed.usage`.
   Quota windows come only from response headers, labelled as undocumented
   (§3.2).

**Out of scope:**

- Routing any subscription on behalf of other users, or sharing logins
  (prohibited, §4).
- Cloaking or impersonating Claude Code or codex-tui, including UA,
  originator or TLS fingerprint spoofing (likely conflicts; unnecessary).
- Running OAuth flows in this task (scope choice, not a provider rule).
- Polling `oauth/usage` or `wham/usage`, and redeeming reset credits.
- A clone of CLIProxyAPI's `/v0/management/api-call`.

**Deferred:**

- Gemini/Antigravity, Grok, Cursor, Kimi, Meta/Muse and Devin. Their t3code
  drivers expose no common base-URL setting, and their subscription terms were
  not researched.
- WebSocket Responses transport.
- Chat Completions translation. Neither t3code CLI needs it.

## 7. Open questions

1. Does the user enable Mode B (own-account pool) after reviewing the
   unresolved terms in §4? Without it, the router recommends accounts but does
   not choose them for subscriptions.
2. Which upstream must a ChatGPT-login Codex request reach in Modes A and B?
   `chatgpt.com/backend-api/codex` is undocumented as a public API.
3. Manual verification is needed once a build exists:
   - t3code Codex account and plan UI with a custom provider;
   - whether a `/backend-api/codex` base URL keeps workspace routing;
   - whether Claude `rate_limit_event` still reaches t3code in pass-through
     mode.
