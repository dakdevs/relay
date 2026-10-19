//! Main window: title-bar tabs (Quotas, Accounts, Usage, Routing) over tonal
//! surfaces. Opened on Quotas by Open Relay; ⌘, opens Routing.

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, KeyBinding,
    ParentElement, Render, Styled, Subscription, Window, WindowControlArea, actions, div,
    prelude::*, px,
};

use crate::{
    accounts::{self, ConnectSheet},
    input::TextInput,
    quotas::{self, Sort},
    routing,
    store::{Command, Store, Tab},
    usage,
    widgets::{self, Button, ButtonKind, ui},
};

actions!(
    dashboard,
    [
        NextTab,
        PrevTab,
        ShowQuotas,
        ShowAccounts,
        ShowUsage,
        ShowRouting,
        CloseWindow
    ]
);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Dashboard");
    cx.bind_keys([
        KeyBinding::new("secondary-1", ShowQuotas, ctx),
        KeyBinding::new("secondary-2", ShowAccounts, ctx),
        KeyBinding::new("secondary-3", ShowUsage, ctx),
        KeyBinding::new("secondary-4", ShowRouting, ctx),
        KeyBinding::new("secondary-,", ShowRouting, ctx),
        KeyBinding::new("ctrl-tab", NextTab, ctx),
        KeyBinding::new("ctrl-shift-tab", PrevTab, ctx),
        KeyBinding::new("secondary-w", CloseWindow, ctx),
    ]);
    crate::input::bind_keys(cx);
}

pub struct Dashboard {
    pub store: Entity<Store>,
    focus: FocusHandle,
    search: Entity<TextInput>,
    sort: Sort,
    pub connect: Option<Entity<ConnectSheet>>,
    _subscriptions: Vec<Subscription>,
    _tick: gpui::Task<()>,
}

impl Focusable for Dashboard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Dashboard {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search by label, email or credential file", cx));
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe(&search, |this, search, cx| {
                let q = search.read(cx).content.to_string();
                this.store.update(cx, |s, cx| {
                    if s.search != q {
                        s.search = q;
                        cx.notify();
                    }
                });
            }),
            crate::app::watch_appearance(window, cx),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let tick = crate::app::keep_times_fresh(&store, cx);
        Self {
            store,
            focus,
            search,
            sort: Sort::Default,
            connect: None,
            _subscriptions: subscriptions,
            _tick: tick,
        }
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.store.update(cx, |s, cx| {
            s.tab = tab;
            cx.notify();
        });
    }

    fn step_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let tab = self.store.read(cx).tab;
        let i = Tab::ALL.iter().position(|t| *t == tab).unwrap_or(0) as isize;
        let n = Tab::ALL.len() as isize;
        self.set_tab(Tab::ALL[((i + delta).rem_euclid(n)) as usize], cx);
    }

    pub fn open_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let sheet = cx.new(|cx| ConnectSheet::new(store, window, cx));
        self.show_sheet(sheet, cx);
    }

    /// Reconnect: the connect sheet on step 2 for an existing account.
    pub fn open_reconnect(
        &mut self,
        account: &crate::model::Account,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let sheet = cx.new(|cx| ConnectSheet::reconnect(store, account, window, cx));
        self.show_sheet(sheet, cx);
    }

    fn show_sheet(&mut self, sheet: Entity<ConnectSheet>, cx: &mut Context<Self>) {
        cx.subscribe(&sheet, |this, _, _: &accounts::Dismiss, cx| {
            this.connect = None;
            cx.notify();
        })
        .detach();
        self.connect = Some(sheet);
        cx.notify();
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = ui(cx).theme;
        let tab = self.store.read(cx).tab;
        let entity = cx.entity();
        let tabs = widgets::segmented(
            "main-tabs",
            &["Quotas", "Accounts", "Usage", "Routing"],
            Tab::ALL.iter().position(|x| *x == tab).unwrap_or(0),
            92.,
            move |i, _, cx| entity.update(cx, |this, cx| this.set_tab(Tab::ALL[i], cx)),
            window,
            cx,
        );
        let store = self.store.clone();
        let trailing = match tab {
            Tab::Quotas => {
                let unsupported = self
                    .store
                    .read(cx)
                    .overview
                    .sections
                    .iter()
                    .flat_map(|s| &s.groups)
                    .flat_map(|g| &g.accounts)
                    .all(|a| a.refresh_unsupported.is_some());
                Some(
                    Button::new("refresh-all", ButtonKind::Secondary, "Refresh all quotas")
                        .icon("icons/refresh.svg")
                        .label("Refresh all")
                        .disabled(unsupported)
                        .on_click(move |_, _, cx| {
                            store.update(cx, |s, cx| {
                                s.send_or_report(Command::RefreshAll, crate::store::Scope::App);
                                cx.notify();
                            })
                        })
                        .into_any_element(),
                )
            }
            // Board 4 puts Connect… in the Accounts header, not the title bar.
            _ => None,
        };

        let title_bar = div()
            .flex_none()
            .h(px(52.))
            .flex()
            .items_center()
            .px(px(16.))
            .bg(t.canvas)
            .window_control_area(WindowControlArea::Drag)
            .relative()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(tabs),
            )
            .child(div().flex_1())
            .children(trailing);

        let body = match tab {
            Tab::Quotas => self.quotas(cx).into_any_element(),
            Tab::Accounts => accounts::view(&self.store, cx.entity(), cx),
            Tab::Usage => usage::view(&self.store, window, cx),
            Tab::Routing => routing::view(&self.store, window, cx),
        };

        div()
            .key_context("Dashboard")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NextTab, _, cx| this.step_tab(1, cx)))
            .on_action(cx.listener(|this, _: &PrevTab, _, cx| this.step_tab(-1, cx)))
            .on_action(cx.listener(|this, _: &ShowQuotas, _, cx| this.set_tab(Tab::Quotas, cx)))
            .on_action(cx.listener(|this, _: &ShowAccounts, _, cx| this.set_tab(Tab::Accounts, cx)))
            .on_action(cx.listener(|this, _: &ShowUsage, _, cx| this.set_tab(Tab::Usage, cx)))
            .on_action(cx.listener(|this, _: &ShowRouting, _, cx| this.set_tab(Tab::Routing, cx)))
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .size_full()
            .flex()
            .flex_col()
            .bg(t.popover)
            .text_color(t.text)
            .text_size(px(13.))
            .relative()
            .child(title_bar)
            .child(
                div()
                    .id("main-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .pt(px(18.))
                    .pb(px(24.))
                    .child(body),
            )
            .children(self.connect.clone().map(|sheet| {
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::hsla(0., 0., 0., if t.dark { 0.45 } else { 0.18 }))
                    .child(sheet)
            }))
    }
}

impl Dashboard {
    fn quotas(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = ui(cx).theme;
        let overview = self.store.read(cx).overview.clone();
        let filter = self.store.read(cx).provider_filter;
        let sort = self.sort;
        if overview.is_empty() {
            return div()
                .child(accounts::empty_main(cx.entity(), &t))
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(quotas::filter_chips(&overview, filter, &self.store, &t))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(480.))
                            .h(px(36.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .rounded(px(9.))
                            .bg(t.track)
                            .child(widgets::icon("icons/search.svg", 14., t.muted))
                            .child(self.search.clone()),
                    )
                    .child(div().flex_1())
                    .child(chip("View", "Ledger", None::<fn(&gpui::ClickEvent, &mut Window, &mut App)>, &t))
                    .child(chip("Sort", sort.label(), Some(cx.listener(|this, _, _, cx| {
                        this.sort = this.sort.next();
                        cx.notify();
                    })), &t)),
            )
            .child(quotas::summary(&overview, &self.store, cx))
            .children(quotas::ledger(&overview, &self.store, sort, cx))
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child("Capacity adds each account's percentage left (N accounts = N × 100%); segments stay per account. Reset limits runs only the provider's own action and never clears Relay's counters. Refresh quota uses a documented usage read, never a billed request."),
            )
            .into_any_element()
    }
}

type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

fn chip(
    label: &'static str,
    value: &'static str,
    on_click: Option<impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>,
    t: &crate::theme::Theme,
) -> impl IntoElement {
    let on_click: Option<ClickHandler> = on_click.map(|f| Box::new(f) as ClickHandler);
    let interactive = on_click.is_some();
    div()
        .id(gpui::ElementId::Name(format!("chip-{label}").into()))
        .role(gpui::Role::Button)
        .aria_label(gpui::SharedString::from(format!("{label}: {value}")))
        .when(interactive, |d| {
            d.focusable().tab_stop(true).cursor_pointer()
        })
        .h(px(36.))
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(9.))
        .bg(t.track)
        .hover(|s| s.bg(t.pressed))
        .focus_visible(|s| s.bg(t.focus))
        .child(div().text_color(t.muted).child(label))
        .child(div().font_weight(FontWeight::MEDIUM).child(value))
        .when(interactive, |d| {
            d.child(div().text_size(px(9.)).text_color(t.muted).child("▾"))
        })
        .when_some(on_click, |d, f| d.on_click(move |e, w, cx| f(e, w, cx)))
}
