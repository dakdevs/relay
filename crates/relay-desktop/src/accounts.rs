//! Accounts tab and connect sheet (board 4). Provider → plan or credential →
//! account, each with its label, masked identifier, plan, state and the one
//! fixing action where one exists. Connecting stores credential *references*
//! only; T3 settings are never read or written — Relay shows copyable values.

use gpui::{
    AnyElement, App, ClipboardItem, Context, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, IntoElement, ParentElement, Render, ScrollHandle, SharedString, Styled,
    Window, div, prelude::*, px,
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
    store::{AccountEdit, Command, ConnectEvent, Op, Scope, Store},
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
                .items_center()
                .gap(px(8.))
                .child(div().text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child("Accounts"))
                .child(div().text_size(px(13.)).text_color(t.muted).child(format!(
                    "{} accounts · {} providers",
                    overview.account_count(),
                    providers
                )))
                .child(div().flex_1())
                .child({
                    let dashboard = dashboard.clone();
                    Button::new("connect", ButtonKind::Primary, "Connect an account")
                        .label("Connect…")
                        .on_click(move |_, window, cx| dashboard.update(cx, |d, cx| d.open_connect(window, cx)))
                }),
        )
        .children(store.read(cx).error(Scope::Accounts).map(|e| {
            div().text_size(px(13.)).text_color(t.ember).child(e.to_string())
        }))
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
                        .children(g.accounts.iter().enumerate().map(|(i, a)| row(a, i % 2 == 0, now, store, &dashboard, &t)))
                }))
        }))
        .child(div().text_size(px(12.)).text_color(t.muted).child(
            "Labels are yours and stay fixed; identifiers are masked. Relay never reads or changes T3's saved logins and counts only traffic that passes through it.",
        ))
        .into_any_element()
}

/// The one action a row offers. Fixing actions (Resume, Reconnect) are
/// raised buttons as on board 4; Pause is a quiet, reversible extra.
#[derive(Clone, Debug, PartialEq)]
pub enum RowAction {
    Resume,
    Reconnect,
    Pause,
}

pub fn row_action(state: State) -> RowAction {
    match state {
        State::Paused => RowAction::Resume,
        State::Disconnected => RowAction::Reconnect,
        _ => RowAction::Pause,
    }
}

fn row(
    a: &Account,
    tinted: bool,
    now: i64,
    store: &Entity<Store>,
    dashboard: &Entity<Dashboard>,
    t: &Theme,
) -> AnyElement {
    let (text, color) = detail(a, now, t);
    let store = store.clone();
    let dashboard = dashboard.clone();
    let account = a.clone();
    let action = row_action(a.state);
    let (label, kind) = match action {
        RowAction::Resume => ("Resume", ButtonKind::Secondary),
        RowAction::Reconnect => ("Reconnect", ButtonKind::Secondary),
        RowAction::Pause => ("Pause", ButtonKind::Quiet),
    };
    let button = Button::new(
        ElementId::Name(format!("fix-{}", a.id).into()),
        kind,
        format!("{label} {}", a.label),
    )
    .label(label)
    .on_click(move |_, window, cx| match action {
        RowAction::Reconnect => {
            dashboard.update(cx, |d, cx| d.open_reconnect(&account, window, cx))
        }
        RowAction::Resume | RowAction::Pause => store.update(cx, |s, cx| {
            s.clear_error(Scope::Accounts);
            s.send_or_report(
                Command::SetAccountPaused {
                    id: account.id.clone(),
                    paused: action == RowAction::Pause,
                },
                Scope::Accounts,
            );
            cx.notify();
        }),
    });
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
        .child(button)
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

/// The connect sheet's state machine, free of UI so its handling of
/// asynchronous replies can be tested deterministically. Replies carry the
/// op of the request they answer; anything else is ignored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectFlow {
    pub step: u8,
    /// The account this sheet saved (or is reconnecting). Once set, the
    /// provider is locked and Continue merges edits onto that account.
    pub saved_id: Option<String>,
    /// Save request awaiting `Saved` or `Failed`.
    pub save_op: Option<Op>,
    /// Setup request whose values step 2 shows.
    pub setup_op: Option<Op>,
    /// Step 1 error: validation, or the failed save's reason.
    pub error: Option<String>,
    /// Step 2 error: the setup request's failure.
    pub setup_error: Option<String>,
    /// Reconnect opens on step 2 with no Back: it is not an edit flow.
    pub reconnect: bool,
    /// `last_seen` when the sheet opened; only a newer request counts.
    pub seen_baseline: Option<i64>,
}

impl ConnectFlow {
    pub fn fresh() -> Self {
        Self {
            step: 1,
            ..Self::default()
        }
    }

    pub fn reconnecting(id: String, setup_op: Op, seen_baseline: Option<i64>) -> Self {
        Self {
            step: 2,
            saved_id: Some(id),
            setup_op: Some(setup_op),
            reconnect: true,
            seen_baseline,
            ..Self::default()
        }
    }

    pub fn provider_locked(&self) -> bool {
        self.saved_id.is_some()
    }

    pub fn saving(&self) -> bool {
        self.save_op.is_some()
    }

    pub fn submitted(&mut self, op: Op) {
        self.save_op = Some(op);
        self.error = None;
    }

    pub fn send_failed(&mut self, reason: String) {
        self.save_op = None;
        self.error = Some(reason);
    }

    pub fn back(&mut self) {
        if !self.reconnect {
            self.step = 1;
            // Replies to the abandoned setup request are now stale.
            self.setup_op = None;
            self.setup_error = None;
        }
    }

    pub fn on_event(&mut self, event: &ConnectEvent) {
        match event {
            ConnectEvent::Saved { op, id } if self.save_op == Some(*op) => {
                self.saved_id = Some(id.clone());
                self.save_op = None;
                self.setup_op = Some(*op);
                self.setup_error = None;
                self.step = 2;
            }
            ConnectEvent::Failed { op, message } if self.save_op == Some(*op) => {
                self.save_op = None;
                self.error = Some(message.clone());
            }
            ConnectEvent::Failed { op, message } if self.setup_op == Some(*op) => {
                self.setup_error = Some(message.clone());
            }
            _ => {}
        }
    }

    /// True once a request newer than the sheet's opening has arrived.
    pub fn first_request_seen(&self, seen_at: Option<i64>) -> bool {
        match (seen_at, self.seen_baseline) {
            (Some(now), Some(base)) => now > base,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

pub struct ConnectSheet {
    store: Entity<Store>,
    focus: FocusHandle,
    provider: Provider,
    choice: Choice,
    label: Entity<TextInput>,
    identifier: Entity<TextInput>,
    credential: Entity<TextInput>,
    flow: ConnectFlow,
    /// The body's scroll state; its max offset says whether it overflows.
    scroll: ScrollHandle,
    _observe: gpui::Subscription,
}

impl EventEmitter<Dismiss> for ConnectSheet {}

impl Focusable for ConnectSheet {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Mode and credential reference for a choice. Secrets are referenced by
/// file path or environment variable name, never stored.
pub fn credential_for(
    provider: Provider,
    choice: Choice,
    credential: &str,
) -> Result<(AccountMode, Option<SecretRef>, Option<CredentialKind>), String> {
    let credential = credential.trim();
    match choice {
        Choice::Observe => Ok((AccountMode::Observed, None, None)),
        Choice::Pool => {
            if credential.is_empty() {
                return Err("Pooling needs the path to this account's own login file.".into());
            }
            let format = match provider {
                Provider::Anthropic => SecretFormat::ClaudeCredentialsJson,
                Provider::Openai => SecretFormat::CodexAuthJson,
            };
            Ok((
                AccountMode::Pooled,
                Some(SecretRef::File {
                    file: credential.into(),
                    format,
                }),
                None,
            ))
        }
        Choice::Key => {
            let var = credential.trim_start_matches('$');
            if var.is_empty() || !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err("Enter the name of the environment variable that holds the key, e.g. ANTHROPIC_API_KEY.".into());
            }
            Ok((
                AccountMode::Key,
                Some(SecretRef::Env {
                    env: var.to_string(),
                }),
                Some(CredentialKind::ApiKey),
            ))
        }
    }
}

/// The text the credential field shows for a saved reference.
pub fn credential_text(credential: Option<&SecretRef>) -> String {
    match credential {
        Some(SecretRef::File { file, .. }) => file.display().to_string(),
        Some(SecretRef::Env { env }) => env.clone(),
        _ => String::new(),
    }
}

/// Builds a new account from the sheet's fields. The identifier is masked
/// before it is stored; secrets are only referenced.
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
    let (mode, credential, credential_kind) = credential_for(provider, choice, credential)?;
    let identifier = identifier.trim();
    Ok(AccountConfig {
        id: AccountId(id),
        provider,
        label: label.to_string(),
        mode,
        plan: None,
        identifier: (!identifier.is_empty()).then(|| mask_identifier(identifier)),
        credential_kind,
        upstream: None,
        credential,
        paused: false,
        max_in_flight: 8,
        models: Vec::new(),
    })
}

/// The editable fields from the sheet, to merge onto a saved account. An
/// empty identifier field keeps the saved (masked) identifier.
pub fn account_edit(
    provider: Provider,
    choice: Choice,
    label: &str,
    identifier: &str,
    credential: &str,
) -> Result<AccountEdit, String> {
    let label = label.trim();
    if label.is_empty() {
        return Err("Give the account a label, such as Personal or Work.".into());
    }
    let (mode, credential, credential_kind) = credential_for(provider, choice, credential)?;
    let identifier = identifier.trim();
    Ok(AccountEdit {
        label: label.to_string(),
        identifier: (!identifier.is_empty()).then(|| mask_identifier(identifier)),
        mode,
        credential,
        credential_kind,
    })
}

impl ConnectSheet {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let label = cx.new(|cx| TextInput::new("Personal", cx));
        let identifier = cx.new(|cx| TextInput::new("you@example.com", cx));
        let credential = cx.new(|cx| TextInput::new("", cx));
        let observe = cx.observe(&store, |this, store, cx| {
            let events = store.read(cx).connect_events.clone();
            for e in &events {
                this.flow.on_event(e);
            }
            cx.notify();
        });
        Self {
            store,
            focus,
            provider: Provider::Anthropic,
            choice: Choice::Observe,
            label,
            identifier,
            credential,
            flow: ConnectFlow::fresh(),
            scroll: ScrollHandle::new(),
            _observe: observe,
        }
    }

    /// Reconnect: step 2 for an existing account, showing its setup values
    /// and waiting for a request newer than now. There is no Back.
    pub fn reconnect(
        store: Entity<Store>,
        account: &Account,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut sheet = Self::new(store, window, cx);
        sheet.provider = account.provider;
        sheet.choice = match account.mode {
            ModeTag::Observed => Choice::Observe,
            ModeTag::Key => Choice::Key,
            ModeTag::Pooled | ModeTag::Mixed => Choice::Pool,
        };
        sheet
            .label
            .update(cx, |l, cx| l.set_text(account.label.clone(), cx));
        let saved_ref = sheet
            .store
            .read(cx)
            .config
            .account(&account.id)
            .map(|a| credential_text(a.credential.as_ref()))
            .unwrap_or_default();
        sheet
            .credential
            .update(cx, |c, cx| c.set_text(saved_ref, cx));
        let id = account.id.clone();
        let seen = account.seen_at;
        let sent = sheet.store.update(cx, |s, _| {
            let op = s.next_op();
            s.send(Command::ClientSetup { op, id: id.clone() })
                .map(|()| op)
        });
        sheet.flow = match sent {
            Ok(op) => ConnectFlow::reconnecting(id, op, seen),
            Err(e) => {
                let mut f = ConnectFlow::reconnecting(id, 0, seen);
                f.setup_error = Some(e);
                f
            }
        };
        sheet
    }

    /// Render fixtures: a sheet in a given state with its fields filled
    /// (label, identifier, credential). Sends nothing.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn fixture(
        store: Entity<Store>,
        provider: Provider,
        choice: Choice,
        flow: ConnectFlow,
        fields: [&str; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut sheet = Self::new(store, window, cx);
        sheet.provider = provider;
        sheet.choice = choice;
        sheet.flow = flow;
        let inputs = [&sheet.label, &sheet.identifier, &sheet.credential];
        for (input, text) in inputs.into_iter().zip(fields) {
            input.update(cx, |i, cx| i.set_text(text.to_string(), cx));
        }
        sheet
    }

    /// How far the body scrolls: non-zero when its content is taller than
    /// the space left above the pinned footer.
    #[doc(hidden)]
    pub fn body_overflow(&self) -> gpui::Pixels {
        self.scroll.max_offset().y
    }

    /// Scrolls the body to its end, as a user would to reach the last field.
    #[doc(hidden)]
    pub fn scroll_body_to_end(&self) {
        let max = self.scroll.max_offset();
        self.scroll.set_offset(gpui::point(px(0.), -max.y));
    }

    /// The credential field, for checking that a long value stays reachable.
    #[doc(hidden)]
    pub fn credential_input(&self) -> &Entity<TextInput> {
        &self.credential
    }

    /// Label, placeholder and helper for the credential field; Observe
    /// has none.
    fn credential_hint(&self) -> Option<(&'static str, &'static str, &'static str)> {
        Some(match (self.provider, self.choice) {
            (_, Choice::Observe) => return None,
            (Provider::Anthropic, Choice::Pool) => (
                "Login file",
                "Path to .credentials.json",
                "That account's own Claude Code login (its CLAUDE_CONFIG_DIR). Relay saves only the path.",
            ),
            (Provider::Openai, Choice::Pool) => (
                "Login file",
                "Path to auth.json",
                "That account's own Codex login (its CODEX_HOME). Relay saves only the path.",
            ),
            (Provider::Anthropic, Choice::Key) => (
                "Environment variable",
                "ANTHROPIC_API_KEY",
                "Relay saves the variable's name, never the key.",
            ),
            (Provider::Openai, Choice::Key) => (
                "Environment variable",
                "OPENAI_API_KEY",
                "Relay saves the variable's name, never the key.",
            ),
        })
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.flow.saving() {
            return;
        }
        let read = |e: &Entity<TextInput>, cx: &Context<Self>| e.read(cx).content.to_string();
        let (label, identifier, credential) = (
            read(&self.label, cx),
            read(&self.identifier, cx),
            read(&self.credential, cx),
        );
        let saved_id = self.flow.saved_id.clone();
        let (provider, choice) = (self.provider, self.choice);
        let built: Result<Box<dyn FnOnce(Op) -> Command>, String> = match &saved_id {
            // After Back: merge the editable fields onto the saved account.
            Some(id) => {
                account_edit(provider, choice, &label, &identifier, &credential).map(|edit| {
                    let id = id.clone();
                    Box::new(move |op| Command::UpdateAccount {
                        op,
                        id,
                        edit: Box::new(edit),
                    }) as Box<dyn FnOnce(Op) -> Command>
                })
            }
            None => {
                let existing = self.store.read(cx).config.accounts.clone();
                account_config(
                    provider,
                    choice,
                    &label,
                    &identifier,
                    &credential,
                    &existing,
                )
                .map(|account| {
                    Box::new(move |op| Command::AddAccount {
                        op,
                        account: Box::new(account),
                    }) as Box<dyn FnOnce(Op) -> Command>
                })
            }
        };
        match built {
            Ok(make) => {
                let sent = self.store.update(cx, |s, _| {
                    let op = s.next_op();
                    s.send(make(op)).map(|()| op)
                });
                match sent {
                    Ok(op) => self.flow.submitted(op),
                    Err(e) => self.flow.send_failed(e),
                }
            }
            Err(e) => self.flow.error = Some(e),
        }
        cx.notify();
    }
}

/// A mode choice: radio, title and one short line. Restrictions that
/// matter for the choice appear only once it is selected.
#[allow(clippy::too_many_arguments)]
fn option_card(
    id: &'static str,
    mark: Option<Mark>,
    title: &'static str,
    body: &'static str,
    detail: (&'static str, gpui::Rgba),
    selected: bool,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
    t: &Theme,
) -> impl IntoElement {
    // Nested fills draw the empty radio as a ring without a stroke.
    let radio = div()
        .mt(px(1.))
        .size(px(18.))
        .flex_none()
        .rounded_full()
        .bg(if selected {
            t.evergreen
        } else {
            widgets::lerp(t.inset, t.faint, 0.6)
        })
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(if selected { 7. } else { 13. }))
                .rounded_full()
                .bg(if selected { t.on_accent } else { t.inset }),
        );
    div()
        .id(id)
        .role(gpui::Role::RadioButton)
        .aria_label(SharedString::from(format!("{title}. {body} {}", detail.0)))
        .aria_toggled(if selected {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .focusable()
        .tab_stop(true)
        .flex()
        .items_start()
        .gap(px(12.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(radius::ROW + 1.))
        .bg(if selected { t.focus } else { t.inset })
        .hover(|s| s.bg(if selected { t.focus } else { t.hover }))
        .focus_visible(|s| s.bg(t.focus))
        .cursor_pointer()
        .on_click(move |_, w, cx| on_select(w, cx))
        .child(radio)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .children(mark.map(|m| widgets::mark(m, 14., t)))
                        .child(
                            div()
                                .min_w(px(0.))
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        ),
                )
                .child(div().text_size(px(12.5)).text_color(t.muted).child(body))
                .when(selected, |d| {
                    d.child(
                        div()
                            .mt(px(4.))
                            .text_size(px(12.))
                            .text_color(detail.1)
                            .child(detail.0),
                    )
                }),
        )
}

/// A labelled text field. The box clips its text, so a long value never
/// paints past the field.
fn field(
    label: &'static str,
    input: &Entity<TextInput>,
    helper: Option<&'static str>,
    t: &Theme,
) -> gpui::Div {
    div()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.muted)
                .child(label),
        )
        .child(
            div()
                .h(px(34.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(8.))
                .bg(t.track)
                .overflow_hidden()
                .child(input.clone()),
        )
        .children(helper.map(|h| div().text_size(px(12.)).text_color(t.faint).child(h)))
}

/// Eyebrow and title. The eyebrow carries user text (the label), so it
/// truncates; the title wraps.
fn heading(eyebrow: String, title: &'static str, t: &Theme) -> impl IntoElement {
    div()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(12.5))
                .text_color(t.muted)
                .truncate()
                .child(eyebrow),
        )
        .child(
            div()
                .text_size(px(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
}

/// A scoped error: wraps inside the sheet on a soft ember fill.
fn error_note(text: String, t: &Theme) -> impl IntoElement {
    div()
        .id("connect-error")
        .role(gpui::Role::Alert)
        .px(px(12.))
        .py(px(9.))
        .rounded(px(radius::ROW))
        .bg(t.tag_ember)
        .text_size(px(12.5))
        .text_color(t.ember)
        .child(text)
}

/// The dimmed backdrop that centres the sheet in the main window. Its
/// padding keeps the sheet off the window edges at any size.
pub fn overlay(sheet: Entity<ConnectSheet>, t: &Theme) -> gpui::Div {
    div()
        .absolute()
        .size_full()
        .p(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0., 0., 0., if t.dark { 0.45 } else { 0.18 }))
        .child(sheet)
}

/// Widest the sheet gets; narrower windows shrink it.
pub const SHEET_MAX_WIDTH: f32 = 540.;

impl Render for ConnectSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = ui(cx).theme;
        let (content, buttons) = if self.flow.step == 1 {
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
            .w_full()
            .max_w(px(SHEET_MAX_WIDTH))
            .max_h_full()
            .flex()
            .flex_col()
            .bg(t.surface)
            .rounded(px(16.))
            .shadow(vec![gpui::BoxShadow {
                color: t.shadow.opacity(t.shadow.a * 1.6),
                offset: gpui::point(px(0.), px(12.)),
                blur_radius: px(32.),
                spread_radius: px(-8.),
                inset: false,
            }])
            // The body scrolls; the footer stays pinned and reachable.
            .child(
                div()
                    .id("connect-body")
                    .track_scroll(&self.scroll)
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .px(px(24.))
                    .pt(px(22.))
                    .pb(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(content),
            )
            // A step 1 error is pinned above the footer so it shows on
            // arrival, however far the body is scrolled.
            .children(
                (self.flow.step == 1)
                    .then(|| self.flow.error.clone())
                    .flatten()
                    .map(|e| {
                        div()
                            .flex_none()
                            .px(px(24.))
                            .pb(px(14.))
                            .child(error_note(e, &t))
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(24.))
                    .py(px(14.))
                    .bg(t.inset)
                    // GPUI doesn't clip children to rounded corners.
                    .rounded_b(px(16.))
                    .children(buttons),
            )
    }
}

impl ConnectSheet {
    fn step_one(&mut self, cx: &mut Context<Self>) -> (AnyElement, Vec<AnyElement>) {
        let t = ui(cx).theme;
        let entity = cx.entity();
        // The provider is part of a saved account's identity: once saved,
        // the picker shows the choice but can't change it.
        let locked = self.flow.provider_locked();
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
                .when(locked, |d| {
                    d.aria_description("Saved account: provider can't change")
                        .opacity(if sel { 1.0 } else { 0.45 })
                })
                .when(!locked, |d| d.focusable().tab_stop(true).cursor_pointer())
                .flex_1()
                .min_w(px(0.))
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(44.))
                .px(px(14.))
                .rounded(px(radius::ROW + 1.))
                .bg(if sel { t.focus } else { t.inset })
                .hover(|s| s.bg(if sel { t.focus } else { t.hover }))
                .focus_visible(|s| s.bg(t.focus))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| {
                        if !this.flow.provider_locked() {
                            this.provider = p;
                            cx.notify();
                        }
                    })
                })
                .child(widgets::mark(provider_mark(p), 18., &t))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(p.label()),
                )
        };
        let choose = |c: Choice, entity: Entity<Self>| {
            move |_: &mut Window, cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.choice = c;
                    cx.notify();
                })
            }
        };
        // The plan mark sits on Pool, the one mode that is about plans.
        let (plan_mark, pool_title, key_title, observe_detail) = match self.provider {
            Provider::Anthropic => (
                Mark::Claude,
                "Pool my own Claude accounts",
                "Use an Anthropic API key",
                (
                    "Documented path. Relay never switches the account.",
                    t.muted,
                ),
            ),
            Provider::Openai => (
                Mark::ChatGpt,
                "Pool my own ChatGPT accounts",
                "Use an OpenAI API key",
                (
                    "Needs a T3 instance with Existing setup. Not yet tested live.",
                    t.amber,
                ),
            ),
        };
        let cred = self.credential_hint();
        if let Some((_, placeholder, _)) = cred {
            self.credential.update(cx, |c, cx| {
                if c.placeholder_text() != placeholder {
                    c.set_placeholder(placeholder, cx);
                }
            });
        }
        let content = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(heading("Step 1 of 2".into(), "Connect an account", &t))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(provider_card(
                        Provider::Anthropic,
                        self.provider == Provider::Anthropic,
                        entity.clone(),
                    ))
                    .child(provider_card(
                        Provider::Openai,
                        self.provider == Provider::Openai,
                        entity.clone(),
                    )),
            )
            .child(
                div()
                    .id("connect-modes")
                    .role(gpui::Role::RadioGroup)
                    .aria_label("Mode")
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(option_card(
                        "choice-observe",
                        None,
                        "Observe a T3 instance",
                        "T3 keeps its own login. Relay records usage and quota.",
                        observe_detail,
                        self.choice == Choice::Observe,
                        choose(Choice::Observe, entity.clone()),
                        &t,
                    ))
                    .child(option_card(
                        "choice-pool",
                        Some(plan_mark),
                        pool_title,
                        "Each new session goes to the account with the most headroom.",
                        (
                            "Opt-in. Running sessions stay on their account. Provider terms neither clearly allow nor forbid pooling one person's own accounts.",
                            t.amber,
                        ),
                        self.choice == Choice::Pool,
                        choose(Choice::Pool, entity.clone()),
                        &t,
                    ))
                    .child(option_card(
                        "choice-key",
                        None,
                        key_title,
                        "Relay reads the key from an environment variable you name.",
                        ("Billed to the key, not to a subscription.", t.muted),
                        self.choice == Choice::Key,
                        choose(Choice::Key, entity.clone()),
                        &t,
                    )),
            )
            .child(
                // Side by side when there's room, stacked when narrow.
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(12.))
                    .child(
                        field("Label", &self.label, Some("Shown in the tray, ledger and usage."), &t)
                            .flex_grow(1.)
                            .flex_basis(px(200.)),
                    )
                    .child(
                        field("Identifier", &self.identifier, Some("Optional. Masked when saved."), &t)
                            .flex_grow(1.)
                            .flex_basis(px(200.)),
                    ),
            )
            .children(cred.map(|(label, _, helper)| field(label, &self.credential, Some(helper), &t)))
            .into_any_element();
        let cancel = cx.listener(|_, _, _, cx| cx.emit(Dismiss));
        let submit = cx.listener(|this, _, _, cx| this.submit(cx));
        let buttons = vec![
            Button::new("connect-cancel", ButtonKind::Secondary, "Cancel")
                .label("Cancel")
                .on_click(cancel)
                .into_any_element(),
            Button::new("connect-continue", ButtonKind::Primary, "Continue")
                .label(if self.flow.saving() {
                    "Saving…"
                } else {
                    "Continue"
                })
                .disabled(self.flow.saving())
                .on_click(submit)
                .into_any_element(),
        ];
        (content, buttons)
    }

    fn step_two(&mut self, cx: &mut Context<Self>) -> (AnyElement, Vec<AnyElement>) {
        let t = ui(cx).theme;
        let store = self.store.read(cx);
        let id = self.flow.saved_id.clone().unwrap_or_default();
        let listening = matches!(
            store.overview.status,
            crate::model::Status::Running | crate::model::Status::Paused
        );
        let address = store.overview.address.clone();
        let setup = self.flow.setup_op.and_then(|op| {
            store.connect_event(op).find_map(|e| match e {
                ConnectEvent::Setup { setup, .. } => Some(setup.clone()),
                _ => None,
            })
        });
        let error = self.flow.setup_error.clone();
        let seen = store
            .overview
            .sections
            .iter()
            .flat_map(|s| &s.groups)
            .flat_map(|g| &g.accounts)
            .find(|a| a.id == id)
            .is_some_and(|a| self.flow.first_request_seen(a.seen_at));
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
                .items_start()
                .gap(px(10.))
                .child(
                    div()
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
                        .flex_1()
                        .min_w(px(0.))
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
        let content = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(heading(
                format!("Step 2 of 2 · {} · {mode} · “{label}”", self.provider.label()),
                title,
                &t,
            ))
            .child(check(
                Some(listening),
                if listening {
                    format!("Relay is listening on {address}")
                } else {
                    format!("Relay is not listening on {address}")
                },
                None,
            ))
            .when(
                self.provider == Provider::Openai && self.choice == Choice::Observe,
                |d| {
                    d.child(check(
                        None,
                        "The T3 instance must use Existing setup".into(),
                        Some("Managed Codex ignores custom provider settings. In T3, use an instance with Existing setup, then add the launch arguments below.".into()),
                    ))
                },
            )
            .child(match (&setup, &error) {
                // Command lines never wrap mid-argument; long ones scroll sideways.
                (Some(s), _) => div()
                    .id("setup-lines")
                    .role(gpui::Role::Document)
                    .aria_label(SharedString::from(s.lines.join(". ")))
                    .bg(t.inset)
                    .rounded(px(radius::ROW))
                    .overflow_x_scroll()
                    .child(
                        div()
                            .p(px(14.))
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap(px(3.))
                            .font_family("Menlo")
                            .text_size(px(12.))
                            .children(
                                s.lines
                                    .iter()
                                    .map(|l| div().flex_none().whitespace_nowrap().child(l.clone())),
                            ),
                    )
                    .into_any_element(),
                (None, Some(e)) => error_note(e.clone(), &t).into_any_element(),
                (None, None) => div()
                    .text_size(px(13.))
                    .text_color(t.muted)
                    .child("Preparing setup values…")
                    .into_any_element(),
            })
            .children(
                setup
                    .iter()
                    .flat_map(|s| s.notes.clone())
                    .map(|n| div().text_size(px(12.5)).text_color(t.muted).child(n)),
            )
            .child(check(
                seen.then_some(true),
                if seen {
                    "First request received".into()
                } else {
                    "Waiting for the first request…".into()
                },
                Some("Run any prompt in that T3 thread. Quota appears after the first response.".into()),
            ))
            .when(self.provider == Provider::Openai, |d| {
                d.child(div().text_size(px(12.5)).text_color(t.amber).child(
                    "Not yet tested live. With a custom provider, T3's own Codex account and plan display may not appear for this instance.",
                ))
            })
            .into_any_element();
        let back = cx.listener(|this, _, _, cx| {
            this.flow.back();
            cx.notify();
        });
        let done = cx.listener(|_, _, _, cx| cx.emit(Dismiss));
        let buttons = [
            // Reconnect isn't an edit flow, so it has no Back.
            (!self.flow.reconnect).then(|| {
                Button::new("connect-back", ButtonKind::Secondary, "Back")
                    .label("Back")
                    .on_click(back)
                    .into_any_element()
            }),
            Some(
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
            ),
            Some(
                Button::new("connect-done", ButtonKind::Primary, "Done")
                    .label("Done")
                    .on_click(done)
                    .into_any_element(),
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        (content, buttons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_take_only_sheet_fields_and_keep_the_identifier_when_blank() {
        let e = account_edit(
            Provider::Openai,
            Choice::Key,
            " Dev laptop ",
            "",
            "$OPENAI_API_KEY",
        )
        .unwrap();
        assert_eq!(e.label, "Dev laptop");
        assert_eq!(
            e.identifier, None,
            "blank keeps the saved masked identifier"
        );
        assert_eq!(
            e.credential,
            Some(SecretRef::Env {
                env: "OPENAI_API_KEY".into()
            })
        );
        let e = account_edit(Provider::Openai, Choice::Observe, "Dev", "dak@acme.co", "").unwrap();
        assert_eq!(e.identifier.as_deref(), Some("d••@acme.co"));
        assert!(account_edit(Provider::Openai, Choice::Observe, " ", "", "").is_err());
    }

    #[test]
    fn saved_references_refill_the_credential_field() {
        assert_eq!(
            credential_text(Some(&SecretRef::Env {
                env: "ANTHROPIC_API_KEY".into()
            })),
            "ANTHROPIC_API_KEY"
        );
        let file = SecretRef::File {
            file: "/home/me/.claude/.credentials.json".into(),
            format: SecretFormat::ClaudeCredentialsJson,
        };
        assert_eq!(
            credential_text(Some(&file)),
            "/home/me/.claude/.credentials.json"
        );
        assert_eq!(credential_text(None), "");
    }

    fn saved(op: Op, id: &str) -> ConnectEvent {
        ConnectEvent::Saved { op, id: id.into() }
    }

    fn failed(op: Op) -> ConnectEvent {
        ConnectEvent::Failed {
            op,
            message: format!("op {op} failed"),
        }
    }

    /// Back → Continue → late replies to the first round, in every order:
    /// only the current request's replies move the sheet.
    #[test]
    fn flow_ignores_stale_replies_in_any_order() {
        let mut f = ConnectFlow::fresh();
        f.submitted(1);
        assert!(f.saving());
        f.on_event(&saved(1, "openai-dev"));
        assert_eq!(
            (f.step, f.setup_op, f.saved_id.as_deref()),
            (2, Some(1), Some("openai-dev"))
        );
        assert!(f.provider_locked());

        f.back();
        assert_eq!((f.step, f.setup_op), (1, None));
        f.submitted(2);
        // A late setup failure for op 1 must not cancel the save of op 2.
        f.on_event(&failed(1));
        assert!(f.saving());
        assert_eq!(f.error, None);
        // A duplicate Saved for op 1 must not advance.
        f.on_event(&saved(1, "openai-dev"));
        assert_eq!(f.step, 1);
        f.on_event(&saved(2, "openai-dev"));
        assert_eq!((f.step, f.setup_op), (2, Some(2)));
        // Re-processing the whole event log is harmless.
        for e in [saved(1, "openai-dev"), failed(1), saved(2, "openai-dev")] {
            f.on_event(&e);
        }
        assert_eq!(
            (f.step, f.setup_op, f.setup_error.as_deref()),
            (2, Some(2), None)
        );
        // The current setup request failing shows on step 2.
        f.on_event(&failed(2));
        assert_eq!(f.setup_error.as_deref(), Some("op 2 failed"));
    }

    #[test]
    fn failed_save_or_send_returns_to_editable_step_one() {
        let mut f = ConnectFlow::fresh();
        f.submitted(5);
        f.on_event(&failed(5));
        assert_eq!(
            (f.step, f.saving(), f.error.as_deref()),
            (1, false, Some("op 5 failed"))
        );
        assert!(!f.provider_locked(), "nothing saved yet");
        f.send_failed("runtime gone".into());
        assert_eq!(
            (f.saving(), f.error.as_deref()),
            (false, Some("runtime gone"))
        );
    }

    #[test]
    fn reconnect_has_no_back_and_counts_only_newer_requests() {
        let mut f = ConnectFlow::reconnecting("openai-dev".into(), 9, Some(1_000));
        f.back();
        assert_eq!(f.step, 2, "no Back on Reconnect");
        assert!(f.provider_locked());
        assert!(
            !f.first_request_seen(Some(1_000)),
            "the old request doesn't count"
        );
        assert!(f.first_request_seen(Some(1_001)));
        assert!(!f.first_request_seen(None));
        assert!(ConnectFlow::fresh().first_request_seen(Some(1)));
    }

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
