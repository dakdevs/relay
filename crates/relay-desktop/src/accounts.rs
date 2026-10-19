//! Accounts tab and connect sheet (board 4). Provider → plan or credential →
//! account, each with its label, masked identifier, plan, state and the one
//! fixing action where one exists. Connecting stores credential *references*
//! only; T3 settings are never read or written — Relay shows copyable values.

use gpui::{
    AnyElement, App, ClipboardItem, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px,
};
use relay_core::{
    config::{AccountConfig, SecretFormat, SecretRef},
    model::{AccountId, AccountMode, CredentialKind, Provider, mask_identifier},
};

use crate::{
    adapter::provider_mark,
    assets::Mark,
    dashboard::Dashboard,
    format,
    input::TextInput,
    model::{Account, ModeTag, State},
    store::{Command, Store},
    theme::{Theme, radius},
    widgets::{self, Button, ButtonKind, Tone, caps, ui},
};

// ---------------------------------------------------------------- table

fn detail(a: &Account, now: i64, t: &Theme) -> (String, gpui::Rgba) {
    let seen = a
        .seen_at
        .map(|s| format::seen_ago(crate::adapter::ms(now - s)));
    match a.state {
        State::Exhausted => {
            let reset = a
                .windows
                .iter()
                .filter(|w| w.left == Some(0.0))
                .filter_map(|w| w.resets_at.map(|r| (w, r)))
                .min_by_key(|(_, r)| *r);
            let text = reset.map_or("Exhausted · reset not reported".to_string(), |(w, r)| {
                format!(
                    "{} resets in {} · {}",
                    w.label,
                    format::duration_short(crate::adapter::ms(r - now)),
                    format::clock(r)
                )
            });
            (text, t.ember)
        }
        State::Stale => (
            a.seen_at
                .map_or("Stale · not in averages".to_string(), |s| {
                    format!(
                        "Last seen {} ago · not in averages",
                        format::duration_short(crate::adapter::ms(now - s))
                    )
                }),
            t.amber,
        ),
        State::Disconnected | State::Paused => (a.note.clone().unwrap_or_default(), t.muted),
        State::Unknown => (
            seen.map_or("Waiting for the first request".into(), |s| {
                format!("No quota reported · {s}")
            }),
            t.muted,
        ),
        State::Healthy | State::Low => (
            format!(
                "{} · {}",
                if a.state == State::Low {
                    "Low"
                } else {
                    "Healthy"
                },
                seen.unwrap_or_else(|| "not seen yet".into())
            ),
            t.muted,
        ),
    }
}

fn state_tag(a: &Account, t: &Theme) -> Option<gpui::Div> {
    Some(match a.state {
        _ if a.in_use > 0 => widgets::tag(format!("● In use · {}", a.in_use), Tone::Strong, t),
        State::Exhausted => widgets::tag("! Exhausted", Tone::Ember, t),
        State::Stale => widgets::tag("◷ Stale", Tone::Amber, t),
        State::Disconnected => widgets::tag("○ Disconnected", Tone::Neutral, t),
        State::Paused => widgets::tag("Ⅱ Paused", Tone::Neutral, t),
        _ => return None,
    })
}

pub fn view(
    store: &Entity<Store>,
    dashboard: Entity<Dashboard>,
    cx: &mut Context<Dashboard>,
) -> AnyElement {
    let t = ui(cx).theme;
    let overview = store.read(cx).overview.clone();
    if overview.is_empty() {
        return empty_main(dashboard, &t);
    }
    let providers = overview.sections.len();
    let now = overview.now;
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .child(div().text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child("Accounts"))
                .child(div().text_size(px(13.)).text_color(t.muted).child(format!(
                    "{} accounts · {} providers",
                    overview.account_count(),
                    providers
                ))),
        )
        .children(overview.sections.iter().map(|sec| {
            widgets::surface(&t)
                .px(px(16.))
                .py(px(16.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(4.))
                        .child(widgets::mark(sec.mark, 18., &t))
                        .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(sec.provider))
                        .child(div().text_size(px(13.)).text_color(t.muted).child(format!("{} accounts", sec.account_count()))),
                )
                .child(
                    div()
                        .flex()
                        .h(px(36.))
                        .items_center()
                        .px(px(28.))
                        .child(caps("Account", t.muted).w(px(250.)))
                        .child(caps("Plan · Credential", t.muted).w(px(220.)))
                        .child(caps("State", t.muted).w(px(190.)))
                        .child(caps("Detail", t.muted)),
                )
                .children(sec.groups.iter().map(|g| {
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .h(px(36.))
                                .px(px(10.))
                                .children(g.mark.map(|m| widgets::mark(m, 15., &t)))
                                .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(g.title.clone()))
                                .child(div().text_size(px(13.)).text_color(t.muted).child(g.count_label.clone()))
                                .child(widgets::tag(g.mode.text(), if g.mode == ModeTag::Observed { Tone::Neutral } else { Tone::Green }, &t)),
                        )
                        .children(g.accounts.iter().enumerate().map(|(i, a)| row(a, i % 2 == 0, now, store, &t)))
                }))
        }))
        .child(div().text_size(px(12.)).text_color(t.muted).child(
            "Labels are yours and stay fixed; identifiers are masked. Relay never reads or changes T3's saved logins and counts only traffic that passes through it.",
        ))
        .into_any_element()
}

fn row(a: &Account, tinted: bool, now: i64, store: &Entity<Store>, t: &Theme) -> AnyElement {
    let (text, color) = detail(a, now, t);
    let action: Option<(&'static str, Command)> = match a.state {
        State::Paused => Some((
            "Resume",
            Command::SetAccountPaused {
                id: a.id.clone(),
                paused: false,
            },
        )),
        State::Disconnected => Some(("Reconnect", Command::ClientSetup(a.id.clone()))),
        // Reversible: a paused account gets no new router-owned sessions;
        // observed clients keep passing through.
        _ => Some((
            "Pause",
            Command::SetAccountPaused {
                id: a.id.clone(),
                paused: true,
            },
        )),
    };
    let store = store.clone();
    div()
        .id(ElementId::Name(SharedString::from(format!(
            "acct-{}",
            a.id
        ))))
        .role(gpui::Role::Row)
        .flex()
        .items_center()
        .min_h(px(54.))
        .px(px(28.))
        .py(px(8.))
        .rounded(px(radius::ROW + 1.))
        .when(tinted, |d| d.bg(t.inset))
        .child(
            div()
                .w(px(250.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(a.label.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(t.muted)
                        .child(a.masked.clone()),
                ),
        )
        .child(
            div()
                .w(px(220.))
                .text_size(px(13.5))
                .text_color(t.muted)
                .child(format!("{} · {}", a.plan, a.credential)),
        )
        .child(div().w(px(190.)).flex().children(state_tag(a, t)))
        .child(
            div()
                .flex_1()
                .text_size(px(13.5))
                .text_color(color)
                .child(text),
        )
        .children(action.map(|(label, command)| {
            Button::new(
                ElementId::Name(format!("fix-{}", a.id).into()),
                ButtonKind::Secondary,
                label,
            )
            .label(label)
            .on_click(move |_, _, cx| store.read(cx).send(command.clone()))
        }))
        .into_any_element()
}

pub fn empty_main(dashboard: Entity<Dashboard>, t: &Theme) -> AnyElement {
    widgets::surface(t)
        .p(px(28.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .max_w(px(640.))
        .child(div().text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child("No accounts yet"))
        .child(div().text_size(px(14.)).text_color(t.muted).child(
            "Connect a subscription or API key, or observe a T3 instance. Relay shows quota as soon as a request passes through.",
        ))
        .child(
            div().mt(px(8.)).flex().child(
                Button::new("connect-first", ButtonKind::Primary, "Connect an account")
                    .label("Connect an account…")
                    .on_click(move |_, window, cx| dashboard.update(cx, |d, cx| d.open_connect(window, cx))),
            ),
        )
        .into_any_element()
}

// ---------------------------------------------------------------- connect sheet

pub struct Dismiss;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Mode A: the T3 instance keeps its login; Relay observes.
    Observe,
    /// Mode B: own-account subscription pool (opt-in, terms unresolved).
    Pool,
    /// Mode D: API key billed to the key owner.
    Key,
}

pub struct ConnectSheet {
    store: Entity<Store>,
    focus: FocusHandle,
    provider: Provider,
    choice: Choice,
    label: Entity<TextInput>,
    identifier: Entity<TextInput>,
    credential: Entity<TextInput>,
    step: u8,
    account_id: Option<String>,
    error: Option<String>,
}

impl EventEmitter<Dismiss> for ConnectSheet {}

impl Focusable for ConnectSheet {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Builds the account config from the sheet's fields. The identifier is
/// masked before it is stored; secrets are only referenced.
pub fn account_config(
    provider: Provider,
    choice: Choice,
    label: &str,
    identifier: &str,
    credential: &str,
    existing: &[AccountConfig],
) -> Result<AccountConfig, String> {
    let label = label.trim();
    if label.is_empty() {
        return Err("Give the account a label, such as Personal or Work.".into());
    }
    let mut base: String = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(48)
        .collect();
    if base.is_empty() {
        base = "account".into();
    }
    let base = format!("{}-{base}", provider.key());
    let mut id = base.clone();
    let mut n = 2;
    while existing.iter().any(|a| a.id.0 == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    let credential = credential.trim();
    let (mode, secret) = match choice {
        Choice::Observe => (AccountMode::Observed, None),
        Choice::Pool => {
            if credential.is_empty() {
                return Err("Pooling needs the path to this account's own login file.".into());
            }
            let format = match provider {
                Provider::Anthropic => SecretFormat::ClaudeCredentialsJson,
                Provider::Openai => SecretFormat::CodexAuthJson,
            };
            (
                AccountMode::Pooled,
                Some(SecretRef::File {
                    file: credential.into(),
                    format,
                }),
            )
        }
        Choice::Key => {
            let var = credential.trim_start_matches('$');
            if var.is_empty() || !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err("Enter the name of the environment variable that holds the key, e.g. ANTHROPIC_API_KEY.".into());
            }
            (
                AccountMode::Key,
                Some(SecretRef::Env {
                    env: var.to_string(),
                }),
            )
        }
    };
    let identifier = identifier.trim();
    Ok(AccountConfig {
        id: AccountId(id),
        provider,
        label: label.to_string(),
        mode,
        plan: None,
        identifier: (!identifier.is_empty()).then(|| mask_identifier(identifier)),
        credential_kind: (choice == Choice::Key).then_some(CredentialKind::ApiKey),
        upstream: None,
        credential: secret,
        paused: false,
        max_in_flight: 8,
        models: Vec::new(),
    })
}

impl ConnectSheet {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let label = cx.new(|cx| TextInput::new("Personal", cx));
        let identifier = cx.new(|cx| TextInput::new("you@example.com (masked when saved)", cx));
        let credential = cx.new(|cx| TextInput::new("", cx));
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        Self {
            store,
            focus,
            provider: Provider::Anthropic,
            choice: Choice::Observe,
            label,
            identifier,
            credential,
            step: 1,
            account_id: None,
            error: None,
        }
    }

    fn credential_hint(&self) -> (&'static str, &'static str) {
        match (self.provider, self.choice) {
            (_, Choice::Observe) => ("", ""),
            (Provider::Anthropic, Choice::Pool) => (
                "Login file for this account",
                "Path to that account's Claude Code .credentials.json (its own CLAUDE_CONFIG_DIR)",
            ),
            (Provider::Openai, Choice::Pool) => (
                "Login file for this account",
                "Path to that account's Codex auth.json (its own CODEX_HOME)",
            ),
            (Provider::Anthropic, Choice::Key) => {
                ("Environment variable with the key", "ANTHROPIC_API_KEY")
            }
            (Provider::Openai, Choice::Key) => {
                ("Environment variable with the key", "OPENAI_API_KEY")
            }
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let read = |e: &Entity<TextInput>, cx: &Context<Self>| e.read(cx).content.to_string();
        let existing = self.store.read(cx).config.accounts.clone();
        match account_config(
            self.provider,
            self.choice,
            &read(&self.label, cx),
            &read(&self.identifier, cx),
            &read(&self.credential, cx),
            &existing,
        ) {
            Ok(account) => {
                let id = account.id.0.clone();
                self.store.update(cx, |s, cx| {
                    s.error = None;
                    s.setup = None;
                    s.send(Command::AddAccount(Box::new(account)));
                    s.send(Command::ClientSetup(id.clone()));
                    cx.notify();
                });
                self.account_id = Some(id);
                self.error = None;
                self.step = 2;
            }
            Err(e) => self.error = Some(e),
        }
        cx.notify();
    }
}

fn option_card(
    id: &'static str,
    title: &'static str,
    badge: (&'static str, Tone),
    body: &'static str,
    selected: bool,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
    t: &Theme,
) -> impl IntoElement {
    div()
        .id(id)
        .role(gpui::Role::RadioButton)
        .aria_label(SharedString::from(format!("{title}. {body}")))
        .aria_toggled(if selected {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .focusable()
        .tab_stop(true)
        .flex()
        .gap(px(12.))
        .p(px(14.))
        .rounded(px(radius::ROW + 1.))
        .bg(if selected { t.focus } else { t.inset })
        .hover(|s| s.bg(if selected { t.focus } else { t.hover }))
        .focus_visible(|s| s.bg(t.focus))
        .cursor_pointer()
        .on_click(move |_, w, cx| on_select(w, cx))
        .child(
            div()
                .mt(px(2.))
                .size(px(18.))
                .flex_none()
                .rounded_full()
                .bg(if selected { t.evergreen } else { t.track })
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(7.)).rounded_full().bg(if selected {
                    t.on_accent
                } else {
                    t.track
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(widgets::tag(badge.0, badge.1, t)),
                )
                .child(div().text_size(px(12.5)).text_color(t.muted).child(body)),
        )
}

fn field(label: &'static str, input: &Entity<TextInput>, t: &Theme) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(12.5)).text_color(t.muted).child(label))
        .child(
            div()
                .h(px(36.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(8.))
                .bg(t.track)
                .child(input.clone()),
        )
}

impl Render for ConnectSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = ui(cx).theme;
        let body = if self.step == 1 {
            self.step_one(cx)
        } else {
            self.step_two(cx)
        };
        div()
            .id("connect-sheet")
            .role(gpui::Role::Dialog)
            .aria_label("Connect an account")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, e: &gpui::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    cx.emit(Dismiss);
                }
            }))
            .w(px(580.))
            .bg(t.surface)
            .rounded(px(16.))
            .shadow(vec![gpui::BoxShadow {
                color: t.shadow.opacity(t.shadow.a * 1.6),
                offset: gpui::point(px(0.), px(12.)),
                blur_radius: px(32.),
                spread_radius: px(-8.),
                inset: false,
            }])
            .overflow_hidden()
            .child(body)
    }
}

impl ConnectSheet {
    fn footer(&self, buttons: Vec<AnyElement>, t: &Theme) -> impl IntoElement {
        div()
            .flex()
            .justify_end()
            .gap(px(8.))
            .px(px(24.))
            .py(px(16.))
            .bg(t.inset)
            .children(buttons)
    }

    fn step_one(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = ui(cx).theme;
        let entity = cx.entity();
        let provider_card = |p: Provider, sel: bool, entity: Entity<Self>| {
            div()
                .id(ElementId::Name(format!("pick-{}", p.key()).into()))
                .role(gpui::Role::RadioButton)
                .aria_label(SharedString::from(p.label()))
                .aria_toggled(if sel {
                    gpui::Toggled::True
                } else {
                    gpui::Toggled::False
                })
                .focusable()
                .tab_stop(true)
                .flex_1()
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(48.))
                .px(px(16.))
                .rounded(px(radius::ROW + 1.))
                .bg(if sel { t.focus } else { t.inset })
                .hover(|s| s.bg(if sel { t.focus } else { t.hover }))
                .focus_visible(|s| s.bg(t.focus))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.provider = p;
                        cx.notify();
                    })
                })
                .child(widgets::mark(provider_mark(p), 18., &t))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(p.label()),
                )
        };
        let (product_mark, product) = match self.provider {
            Provider::Anthropic => (
                Mark::Claude,
                "Claude subscription · Max or Pro, or a Console API key",
            ),
            Provider::Openai => (
                Mark::ChatGpt,
                "ChatGPT plan used by Codex, or a Platform API key",
            ),
        };
        let choose = |c: Choice, entity: Entity<Self>| {
            move |_: &mut Window, cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.choice = c;
                    cx.notify();
                })
            }
        };
        let observe_badge = match self.provider {
            Provider::Anthropic => ("Documented path", Tone::Neutral),
            Provider::Openai => ("T3 Existing setup · untested", Tone::Neutral),
        };
        let (cred_label, cred_hint) = self.credential_hint();
        if !cred_hint.is_empty() {
            self.credential.update(cx, |c, cx| {
                if c.placeholder_text() != cred_hint {
                    c.set_placeholder(cred_hint, cx);
                }
            });
        }
        let cancel = cx.listener(|_, _, _, cx| cx.emit(Dismiss));
        let submit = cx.listener(|this, _, _, cx| this.submit(cx));
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .p(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(div().text_size(px(12.5)).text_color(t.muted).child("Step 1 of 2"))
                    .child(div().mt(px(-10.)).text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child("Connect an account"))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(provider_card(Provider::Anthropic, self.provider == Provider::Anthropic, entity.clone()))
                            .child(provider_card(Provider::Openai, self.provider == Provider::Openai, entity.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(widgets::mark(product_mark, 15., &t))
                            .child(div().text_size(px(13.)).text_color(t.muted).child(product)),
                    )
                    .child(option_card(
                        "choice-observe",
                        "Observe a T3 instance",
                        observe_badge,
                        "The instance keeps its own login. Relay records usage and quota and never switches the account.",
                        self.choice == Choice::Observe,
                        choose(Choice::Observe, entity.clone()),
                        &t,
                    ))
                    .child(option_card(
                        "choice-pool",
                        "Pool my own subscription accounts",
                        ("Opt-in · terms unresolved", Tone::Amber),
                        "Relay gives each new session the account with the most headroom; running sessions stay on their account. Provider terms neither clearly allow nor forbid this for one person's own accounts.",
                        self.choice == Choice::Pool,
                        choose(Choice::Pool, entity.clone()),
                        &t,
                    ))
                    .child(option_card(
                        "choice-key",
                        "Use an API key",
                        ("Billed to the key", Tone::Neutral),
                        "Relay reads the key from an environment variable you name. Usage is billed to that key, not to a subscription.",
                        self.choice == Choice::Key,
                        choose(Choice::Key, entity.clone()),
                        &t,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(field("Label (shown everywhere)", &self.label, &t))
                            .child(field("Identifier shown as", &self.identifier, &t)),
                    )
                    .when(self.choice != Choice::Observe, |d| d.child(field(cred_label, &self.credential, &t)))
                    .children(self.error.clone().map(|e| div().text_size(px(13.)).text_color(t.ember).child(e))),
            )
            .child(self.footer(
                vec![
                    Button::new("connect-cancel", ButtonKind::Secondary, "Cancel").label("Cancel").on_click(cancel).into_any_element(),
                    Button::new("connect-continue", ButtonKind::Primary, "Continue").label("Continue").on_click(submit).into_any_element(),
                ],
                &t,
            ))
            .into_any_element()
    }

    fn step_two(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = ui(cx).theme;
        let store = self.store.read(cx);
        let id = self.account_id.clone().unwrap_or_default();
        let listening = matches!(
            store.overview.status,
            crate::model::Status::Running | crate::model::Status::Paused
        );
        let address = store.overview.address.clone();
        let setup = store.setup.clone().filter(|s| s.account_id == id);
        let error = store.error.clone();
        let seen = store
            .overview
            .sections
            .iter()
            .flat_map(|s| &s.groups)
            .flat_map(|g| &g.accounts)
            .find(|a| a.id == id)
            .is_some_and(|a| a.seen_at.is_some());
        let label = self.label.read(cx).content.to_string();
        let mode = match self.choice {
            Choice::Observe => "Observe",
            Choice::Pool => "Pool",
            Choice::Key => "API key",
        };
        let title = match (self.provider, self.choice) {
            (Provider::Openai, Choice::Observe) => "Point a T3 Codex instance at Relay",
            (Provider::Anthropic, Choice::Observe) => "Point a T3 Claude instance at Relay",
            _ => "Point your client at Relay",
        };
        let check = |ok: Option<bool>, text: String, sub: Option<String>| {
            let (bg, glyph) = match ok {
                Some(true) => (t.evergreen, "✓"),
                Some(false) => (t.ember, "!"),
                None => (t.track, "·"),
            };
            div()
                .flex()
                .gap(px(10.))
                .child(
                    div()
                        .mt(px(1.))
                        .size(px(20.))
                        .flex_none()
                        .rounded_full()
                        .bg(bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if ok.is_some() {
                            gpui::rgb(0xFFFFFF)
                        } else {
                            t.muted
                        })
                        .child(glyph),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(14.)).child(text))
                        .children(
                            sub.map(|s| div().text_size(px(12.5)).text_color(t.muted).child(s)),
                        ),
                )
        };
        let copy_text = setup.as_ref().map(|s| s.lines.join("\n"));
        let back = cx.listener(|this, _, _, cx| {
            this.step = 1;
            cx.notify();
        });
        let done = cx.listener(|_, _, _, cx| cx.emit(Dismiss));
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .p(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(div().text_size(px(12.5)).text_color(t.muted).child(format!(
                        "Step 2 of 2 · {} · {mode} · “{label}”",
                        self.provider.label()
                    )))
                    .child(div().mt(px(-10.)).text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(check(
                        Some(listening),
                        if listening { format!("Relay is listening on {address}") } else { format!("Relay is not listening on {address}") },
                        None,
                    ))
                    .when(self.provider == Provider::Openai && self.choice == Choice::Observe, |d| {
                        d.child(check(
                            None,
                            "The T3 instance must use Existing setup".into(),
                            Some("Managed Codex ignores custom provider settings. In T3, use an instance with Existing setup, then add the launch arguments below.".into()),
                        ))
                    })
                    .child(match (&setup, &error) {
                        (Some(s), _) => div()
                            .id("setup-lines")
                            .role(gpui::Role::Document)
                            .aria_label(SharedString::from(s.lines.join(". ")))
                            .bg(t.inset)
                            .rounded(px(9.))
                            .p(px(14.))
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .font_family("Menlo")
                            .text_size(px(12.))
                            .children(s.lines.iter().map(|l| div().child(l.clone())))
                            .into_any_element(),
                        (None, Some(e)) => div().text_size(px(13.)).text_color(t.ember).child(e.clone()).into_any_element(),
                        (None, None) => div().text_size(px(13.)).text_color(t.muted).child("Preparing setup values…").into_any_element(),
                    })
                    .children(setup.iter().flat_map(|s| s.notes.clone()).map(|n| div().text_size(px(12.5)).text_color(t.muted).child(n)))
                    .child(check(
                        seen.then_some(true),
                        if seen { "First request received".into() } else { "Waiting for the first request…".into() },
                        Some("Run any prompt in that T3 thread. Quota appears after the first response.".into()),
                    ))
                    .when(self.provider == Provider::Openai, |d| {
                        d.child(div().text_size(px(12.5)).text_color(t.amber).child(
                            "Not yet tested live. With a custom provider, T3's own Codex account and plan display may not appear for this instance.",
                        ))
                    }),
            )
            .child(self.footer(
                vec![
                    Button::new("connect-back", ButtonKind::Secondary, "Back").label("Back").on_click(back).into_any_element(),
                    Button::new("connect-copy", ButtonKind::Secondary, "Copy setup values")
                        .label("Copy values")
                        .icon("icons/copy.svg")
                        .disabled(copy_text.is_none())
                        .on_click(move |_, _, cx| {
                            if let Some(text) = &copy_text {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                            }
                        })
                        .into_any_element(),
                    Button::new("connect-done", ButtonKind::Primary, "Done").label("Done").on_click(done).into_any_element(),
                ],
                &t,
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_account_has_no_credential_and_masked_identifier() {
        let a = account_config(
            Provider::Openai,
            Choice::Observe,
            " Dev ",
            "dak@acme.co",
            "",
            &[],
        )
        .unwrap();
        assert_eq!(a.id.0, "openai-dev");
        assert_eq!(a.label, "Dev");
        assert_eq!(a.mode, AccountMode::Observed);
        assert_eq!(a.credential, None);
        assert_eq!(a.identifier.as_deref(), Some("d••@acme.co"));
        assert!(AccountId::is_valid(&a.id.0));
    }

    #[test]
    fn ids_are_unique_and_valid() {
        let first = account_config(
            Provider::Anthropic,
            Choice::Observe,
            "Work · Main!",
            "",
            "",
            &[],
        )
        .unwrap();
        assert_eq!(first.id.0, "anthropic-work---main");
        assert!(AccountId::is_valid(&first.id.0));
        let second = account_config(
            Provider::Anthropic,
            Choice::Observe,
            "Work · Main!",
            "",
            "",
            std::slice::from_ref(&first),
        )
        .unwrap();
        assert_eq!(second.id.0, "anthropic-work---main-2");
    }

    #[test]
    fn pool_and_key_store_references_never_values() {
        let pool = account_config(
            Provider::Anthropic,
            Choice::Pool,
            "Personal",
            "",
            "/home/me/.claude-personal/.credentials.json",
            &[],
        )
        .unwrap();
        assert_eq!(pool.mode, AccountMode::Pooled);
        assert!(matches!(
            pool.credential,
            Some(SecretRef::File {
                format: SecretFormat::ClaudeCredentialsJson,
                ..
            })
        ));
        let key = account_config(
            Provider::Openai,
            Choice::Key,
            "Key A",
            "",
            "$OPENAI_API_KEY",
            &[],
        )
        .unwrap();
        assert_eq!(
            key.credential,
            Some(SecretRef::Env {
                env: "OPENAI_API_KEY".into()
            })
        );
        assert_eq!(key.credential_kind, Some(CredentialKind::ApiKey));
        // A pasted secret is not a valid variable name, so it is refused rather than stored.
        assert!(
            account_config(
                Provider::Openai,
                Choice::Key,
                "Key A",
                "",
                "sk-proj-abc-123",
                &[]
            )
            .is_err()
        );
        assert!(account_config(Provider::Anthropic, Choice::Pool, "P", "", " ", &[]).is_err());
        assert!(account_config(Provider::Anthropic, Choice::Observe, "  ", "", "", &[]).is_err());
    }
}
