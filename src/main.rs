pub mod db;
pub mod settings;
pub mod take_buy;
pub mod take_sell;
pub mod util;

use crate::db::{connect, User};
use crate::settings::{get_settings_path, init_global_settings, Settings};
use crate::take_buy::take_buy;
use crate::take_sell::take_sell;
use crate::util::order_from_tags;

use chrono::{DateTime, Local, TimeZone};
use mostro_core::message::{Action, Message, Payload};
use mostro_core::order::{Kind as OrderKind, SmallOrder as Order, Status};
use mostro_core::NOSTR_REPLACEABLE_EVENT_KIND;
use nostr_sdk::prelude::*;
use ratatui::layout::Flex;
use ratatui::style::Color;
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, EventStream, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::palette::tailwind::{BLUE, SLATE},
    style::{Style, Stylize},
    text::Line,
    widgets::{
        Block, Cell, HighlightSpacing, Row, StatefulWidget, Table, TableState, Tabs, Widget,
    },
    DefaultTerminal, Frame,
};
use std::cmp::Ordering;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
mod widgets;
use widgets::settings_widget::SettingsWidget;

static SETTINGS: OnceLock<Settings> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    let settings_path = get_settings_path();
    let settings_file_path = PathBuf::from(settings_path);

    // Create config global var
    init_global_settings(Settings::new(settings_file_path)?);
    let terminal = ratatui::init();
    let mostro = PublicKey::from_str(Settings::get().mostro_pubkey.as_str())?;
    let pool = connect().await?;

    let identity_keys = User::get_identity_keys(&pool)
        .await
        .map_err(|e| format!("Failed to get identity keys: {}", e))?;

    let (trade_keys, trade_index) = User::get_next_trade_keys(&pool)
        .await
        .map_err(|e| format!("Failed to get trade keys: {}", e))?;
    let user = User::get(&pool).await?;
    let app = App::new(
        mostro,
        identity_keys,
        trade_keys,
        trade_index,
        user.mnemonic,
    );
    // Call function to connect to relays
    let client = util::connect_nostr().await?;

    let since_time = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(7))
        .unwrap()
        .timestamp() as u64;
    let timestamp = Timestamp::from(since_time);

    let filters = Filter::new()
        .author(mostro)
        .limit(20)
        .since(timestamp)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), vec!["mostro"])
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"])
        .kind(Kind::Custom(NOSTR_REPLACEABLE_EVENT_KIND));
    // Here subscribe to get orders
    let orders_sub_id = SubscriptionId::new("orders-sub-id");
    client
        .subscribe_with_id(orders_sub_id, vec![filters], None)
        .await?;

    // Here subscribe to get messages
    // let messages_sub_id = SubscriptionId::new("messages-sub-id");
    // let filter = Filter::new()
    //     .pubkey(app.my_keys.public_key())
    //     .kinds([Kind::GiftWrap, Kind::PrivateDirectMessage])
    //     .since(timestamp);
    // client
    //     .subscribe_with_id(messages_sub_id, vec![filter], None)
    //     .await?;
    let app_result = app.run(terminal, client).await;
    ratatui::restore();

    app_result
}

#[derive(Debug)]
struct App {
    trade_keys: Keys,
    identity_keys: Keys,
    mnemonic: String,
    trade_index: i64,
    mostro_pubkey: PublicKey,
    should_quit: bool,
    show_order: bool,
    selected_tab: usize,
    orders: OrderListWidget,
    messages: MostroListWidget,
    show_amount_input: bool,
    show_invoice_input: bool,
    amount_input: Input,
}

impl App {
    const FRAMES_PER_SECOND: f32 = 60.0;

    pub fn new(
        mostro_pubkey: PublicKey,
        identity_keys: Keys,
        trade_keys: Keys,
        trade_index: i64,
        mnemonic: String,
    ) -> Self {
        let amount_input = Input::default();

        Self {
            identity_keys,
            trade_keys,
            trade_index,
            mostro_pubkey,
            mnemonic,
            should_quit: false,
            show_order: false,
            selected_tab: 0,
            orders: OrderListWidget::default(),
            messages: MostroListWidget::default(),
            show_amount_input: false,
            show_invoice_input: false,
            amount_input,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal, client: Client) -> Result<()> {
        self.orders.run(client.clone());
        self.messages.run(client.clone(), self.trade_keys.clone());

        let period = Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => { terminal.draw(|frame| self.draw(frame))?; },
                Some(Ok(event)) = events.next() => self.handle_event(&event, client.clone()).await,
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]);
        let [tabs_area, body_area] = vertical.areas(frame.area());

        // Defining tabs labels
        let tab_titles = ["Orders", "My Trades", "Messages", "Settings"]
            .iter()
            .map(|t| Line::from(*t).bold())
            .collect::<Vec<Line>>();
        let color = Color::from_str("#304F00").unwrap();

        let tabs = Tabs::new(tab_titles)
            .block(Block::bordered().title(" Mostro "))
            .bg(color)
            .select(self.selected_tab)
            .highlight_style(Style::new().fg(BLUE.c400));

        frame.render_widget(tabs, tabs_area);

        match self.selected_tab {
            0 => self.render_orders_tab(frame, body_area),
            1 => self.render_text_tab(frame, body_area, "My Trades"),
            2 => self.render_messages_tab(frame, body_area),
            3 => self.render_settings_tab(frame, body_area),
            _ => {}
        }

        if self.show_invoice_input {
            let popup_area = popup_area(frame.area(), 50, 20);
            let block = Block::bordered().title("Invoice input").bg(Color::Black);
            let lines = vec![
                Line::raw("🧌 You took this selling order, please use a fiat payment processor that allows you to send the money immediately and in which there is no risk of freezing funds."),
                Line::raw("If, for any reason, your payment processor puts the payment on pause and the funds do not arrive in less than 22 hours, the sats will return to the seller, putting the buyer at risk and I cannot force the seller to send the sats again."),
                Line::raw("If you agree with the above, enter a lightning invoice."),
            ];
            let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
            let input_paragraph = Paragraph::new(vec![Line::from(self.amount_input.value())])
                .block(Block::default().borders(ratatui::widgets::Borders::ALL))
                .wrap(Wrap { trim: true });
            frame.render_widget(Clear, popup_area);
            frame.render_widget(paragraph, popup_area);

            // Render input
            frame.render_widget(
                input_paragraph,
                Rect::new(popup_area.x, popup_area.y + 4, popup_area.width, 3),
            );
        }

        if self.show_amount_input {
            let popup_area = popup_area(frame.area(), 55, 25);
            let color: Color = Color::from_str("#14161C").unwrap();
            let block = Block::bordered()
                .title("Amount input".to_string())
                .bg(color)
                .title_style(Style::new().fg(Color::White))
                .title_bottom(format!("ESC to close, ENTER to send fiat amount"));
            let selected = self.orders.state.read().unwrap().table_state.selected();
            let state = self.orders.state.read().unwrap();
            let order = match selected {
                Some(i) => state.orders.get(i).unwrap(),
                None => return,
            };
            let lines = vec![
                Line::raw("This is a range order."),
                Line::raw(format!(
                    "Please enter an amount between {} {}.",
                    order.fiat_amount(),
                    order.fiat_code
                )),
            ];

            let paragraph = Paragraph::new(lines)
                .block(block)
                .cyan()
                .wrap(Wrap { trim: true });
            let input_paragraph = Paragraph::new(vec![Line::from(self.amount_input.value())])
                .block(Block::default().borders(ratatui::widgets::Borders::ALL))
                .wrap(Wrap { trim: true });
            frame.render_widget(Clear, popup_area);
            frame.render_widget(paragraph, popup_area);

            // Render input
            frame.render_widget(
                input_paragraph,
                Rect::new(popup_area.x, popup_area.y + 5, popup_area.width, 3),
            );
        }

        if self.show_order {
            let popup_area = popup_area(frame.area(), 50, 60);
            let selected = self.orders.state.read().unwrap().table_state.selected();
            let state = self.orders.state.read().unwrap();
            let order = match selected {
                Some(i) => state.orders.get(i).unwrap(),
                None => return,
            };
            let action = match order.kind {
                Some(OrderKind::Buy) => "Sell",
                Some(OrderKind::Sell) => "Buy",
                _ => "Trade",
            };
            let order_type = match order.kind {
                Some(OrderKind::Buy) => "Buying",
                Some(OrderKind::Sell) => "Selling",
                _ => "Trading",
            };
            let color: Color = Color::from_str("#14161C").unwrap();
            let block = Block::bordered()
                .title("Order details".to_string())
                .bg(color)
                .title_style(Style::new().fg(Color::White))
                .title_bottom(format!("ESC to close, ENTER to {}", action));
            let sats_amount = order.sats_amount();
            let premium = match order.premium.cmp(&0) {
                Ordering::Equal => "No premium or discount".to_string(),
                Ordering::Less => format!("a {}% discount", order.premium),
                Ordering::Greater => format!("a {}% premium", order.premium),
            };
            let fiat_amount = order.fiat_amount();
            let created_at: DateTime<Local> =
                Local.timestamp_opt(order.created_at.unwrap(), 0).unwrap();
            let lines = vec![
                Line::raw(format!(
                    "Someone is {} sats for {} {} at {} with {}.",
                    order_type, fiat_amount, order.fiat_code, sats_amount, premium
                )),
                Line::raw(""),
                Line::raw(format!("The payment method is {}.", order.payment_method)),
                Line::raw(""),
                Line::raw(format!("Id: {}", order.id.unwrap())),
                Line::raw(""),
                Line::raw(format!("Created at: {}", created_at)),
            ];
            let paragraph = Paragraph::new(lines)
                .block(block)
                .cyan()
                .wrap(Wrap { trim: true });

            frame.render_widget(Clear, popup_area);
            frame.render_widget(paragraph, popup_area);
        }
    }

    fn render_orders_tab(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(&self.orders, area);
    }

    fn render_messages_tab(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(&self.messages, area);
    }

    fn render_text_tab(&self, frame: &mut Frame, area: Rect, text: &str) {
        let text_line = Line::from(text).centered();
        frame.render_widget(text_line, area);
    }

    fn render_settings_tab(&self, frame: &mut Frame, area: Rect) {
        let settings_widget = SettingsWidget::new(self.mostro_pubkey, self.mnemonic.clone());
        frame.render_widget(settings_widget, area);
    }

    async fn take_order(&mut self, order: Order, action: Action, client: &Client) -> Result<()> {
        if self.show_amount_input {
            match self.amount_input.value().parse::<i64>() {
                Ok(value) => {
                    if value >= order.min_amount.unwrap_or(10)
                        && value <= order.max_amount.unwrap_or(500)
                    {
                        self.show_amount_input = false;
                        self.show_order = false;

                        // TODO: Implement https://mostro.network/protocol/key_management.html
                        let order_id = match order.id {
                            Some(id) => id,
                            None => {
                                println!("Order ID is missing");
                                return Err("Order ID is missing".into());
                            }
                        };
                        let message = Message::new_order(
                            Some(order_id),
                            None,
                            Some(self.trade_index),
                            action,
                            Some(Payload::Amount(value)),
                        )
                        .as_json()
                        .map_err(|e| format!("Error serializing message to JSON: {}", e))?;
                        let sig = Message::sign(message.clone(), &self.trade_keys);
                        // We compose the content
                        let content = serde_json::to_string(&(message, sig)).unwrap();
                        // We create the rumor
                        let rumor = EventBuilder::text_note(content)
                            .pow(0)
                            .build(self.trade_keys.public_key());
                        println!(
                            "Taking the order {} for {} {} with payment method {}",
                            order.id.unwrap(),
                            value,
                            order.fiat_code,
                            order.payment_method
                        );
                        let event = EventBuilder::gift_wrap(
                            &self.trade_keys,
                            &self.mostro_pubkey,
                            rumor,
                            [],
                        )
                        .await?;

                        let msg = ClientMessage::event(event);
                        client.send_msg_to(Settings::get().relays, msg).await?;
                    } else {
                        self.show_amount_input = false;
                        println!("Value {} is out of the order range", value);
                    }
                }
                Err(_) => {
                    self.show_amount_input = false;
                    println!("Invalid amount input. Please enter a valid number.");
                }
            }
        } else if self.show_order {
            if order.max_amount.is_some() {
                self.show_amount_input = true;
                self.show_order = false;
            } else {
                // TODO: Implement https://mostro.network/protocol/key_management.html
                let order_id = match order.id {
                    Some(id) => id,
                    None => {
                        println!("Order ID is missing");
                        return Err("Order ID is missing".into());
                    }
                };
                let message =
                    Message::new_order(Some(order_id), None, Some(self.trade_index), action, None)
                        .as_json()
                        .map_err(|e| format!("Error serializing message to JSON: {}", e))?;

                let sig = Message::sign(message.clone(), &self.trade_keys);
                // We compose the content
                let content = serde_json::to_string(&(message, sig)).unwrap();
                // We create the rumor
                let rumor = EventBuilder::text_note(content)
                    .pow(0)
                    .build(self.trade_keys.public_key());

                println!(
                    "Taking the order {} for {} {}, with payment method {}",
                    order.id.unwrap(),
                    order.fiat_amount,
                    order.fiat_code,
                    order.payment_method
                );

                let event =
                    EventBuilder::gift_wrap(&self.trade_keys, &self.mostro_pubkey, rumor, [])
                        .await?;

                let msg = ClientMessage::event(event);
                client.send_msg_to(Settings::get().relays, msg).await?;
                self.show_order = false;
            }
        } else {
            self.show_order = true;
        }

        Ok(())
    }

    async fn handle_event(&mut self, event: &Event, client: Client) {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Char('j') | KeyCode::Down => {
                        if self.selected_tab == 2 {
                            self.messages.scroll_down();
                        } else {
                            self.orders.scroll_down();
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.selected_tab == 2 {
                            self.messages.scroll_up();
                        } else {
                            self.orders.scroll_up();
                        }
                    }
                    KeyCode::Left => {
                        if self.selected_tab > 0 {
                            self.selected_tab -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if self.selected_tab < 3 {
                            self.selected_tab += 1;
                            self.show_order = false;
                            self.show_amount_input = false;
                        }
                    }
                    KeyCode::Enter => {
                        if self.selected_tab == 0
                            || self.show_order
                            || self.show_amount_input
                            || self.show_invoice_input
                        {
                            let order = {
                                let state = self.orders.state.read().unwrap();
                                let selected = state.table_state.selected();
                                selected.and_then(|i| state.orders.get(i).cloned())
                            };

                            if let Some(order) = order {
                                if let Some(kind) = order.kind {
                                    match kind {
                                        OrderKind::Sell => {
                                            take_sell(order, &client);
                                        }
                                        OrderKind::Buy => {
                                            take_buy(order, &client);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Esc => {
                        self.show_order = false;
                        self.show_amount_input = false;
                    }
                    _ => {
                        if self.show_amount_input {
                            self.amount_input.handle_event(&Event::Key(*key)); // Handle keyboard events in textarea
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct MostroListWidget {
    state: Arc<RwLock<MostroListState>>,
}

#[derive(Debug, Default)]
struct MostroListState {
    messages: Vec<DM>,
    loading_state: LoadingState,
    table_state: TableState,
}

#[derive(Debug)]
struct DM {
    id: String,
    kind: Kind,
    sender: PublicKey,
    content: String,
    created_at: u64,
}

impl MostroListWidget {
    /// Start fetching the orders in the background.
    ///
    /// This method spawns a background task that fetches the orders from the Nostr relay.
    fn run(&self, client: Client, my_keys: Keys) {
        let this = self.clone();
        tokio::spawn(this.fetch_dms(client, my_keys));
    }

    async fn fetch_dms(self, client: Client, my_keys: Keys) {
        self.set_loading_state(LoadingState::Loading);

        client
            .handle_notifications(move |notification| {
                let this = self.clone();
                let my_keys = my_keys.clone();
                async move {
                    if let RelayPoolNotification::Event {
                        subscription_id,
                        event,
                        ..
                    } = notification
                    {
                        if subscription_id == SubscriptionId::new("messages-sub-id")
                            && event.kind == Kind::GiftWrap
                            || event.kind == Kind::PrivateDirectMessage
                        {
                            this.handle_message_event(*event, my_keys)?;
                        }
                    }
                    Ok(false)
                }
            })
            .await
            .unwrap();
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    fn scroll_down(&self) {
        self.state.write().unwrap().table_state.scroll_down_by(1);
    }

    fn scroll_up(&self) {
        self.state.write().unwrap().table_state.scroll_up_by(1);
    }

    fn handle_message_event(&self, event: nostr_sdk::Event, my_keys: Keys) -> Result<()> {
        // match event.kind {
        //     Kind::GiftWrap => {
        //         let unwrapped_gift = match unwrap_gift_wrap(Some(&my_keys), None, None, &event) {
        //             Ok(u) => u,
        //             Err(_) => {
        //                 return Err("Error unwrapping gift".into());
        //             }
        //         };
        //         let dm = DM {
        //             id: event.id.to_string(),
        //             kind: event.kind,
        //             sender: unwrapped_gift.sender,
        //             content: unwrapped_gift.rumor.content.clone(),
        //             created_at: unwrapped_gift.rumor.created_at.as_u64(),
        //         };
        //         let mut state = self.state.write().unwrap();
        //         state.messages.retain(|m| m.id != dm.id);

        //         state.messages.push(dm);
        //         state.loading_state = LoadingState::Loaded;

        //         if !state.messages.is_empty() {
        //             state.table_state.select(Some(0));
        //         }
        //         // Handle possible messages from mostro
        //         let message = Message::from_json(&unwrapped_gift.rumor.content).unwrap();
        //         match message.get_inner_message_kind().action {
        //             Action::AddInvoice => {
        //                 // TODO: find a way of get a mutable reference to app
        //                 // app.show_invoice_input = true;
        //             }
        //             Action::NewOrder => {
        //                 todo!("New order created message");
        //             }
        //             Action::CantDo => {
        //                 println!("CantDo message");
        //             }
        //             Action::Rate => {
        //                 println!("Rate message");
        //             }
        //             _ => {}
        //         }
        //     }
        //     Kind::PrivateDirectMessage => todo!("Handle PrivateDirectMessage"),
        //     _ => {}
        // }

        Ok(())
    }
}

impl Widget for &MostroListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // A block with a right-aligned title with the loading state on the right
        let loading_state = Line::from(format!("{:?}", state.loading_state)).right_aligned();
        let color: Color = Color::from_str("#1D212C").unwrap();
        let block = Block::bordered()
            .title(" DMs ")
            .title(loading_state)
            .bg(color)
            .title_bottom("j/k to scroll, ENTER to select order, q to quit");
        // A table with the list of orders
        let rows = state.messages.iter().map(|dm| {
            let sender = if dm.sender
                == PublicKey::from_str(Settings::get().mostro_pubkey.as_str()).unwrap()
            {
                "Mostro".to_string()
            } else {
                dm.sender.to_string()
            };
            let content = if dm.kind == Kind::GiftWrap {
                let message = Message::from_json(&dm.content).unwrap();
                message.get_inner_message_kind().action.to_string()
            } else {
                dm.content.clone()
            };
            let created_at = Local.timestamp_opt(dm.created_at as i64, 0).unwrap();
            Row::new(vec![sender, content, created_at.to_string()])
        });
        let widths = [
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ];
        let color = Color::from_str("#304F00").unwrap();
        let header_style = Style::default().fg(SLATE.c200).bg(color);
        let selected_style = Style::default().fg(BLUE.c400);
        let header = ["Sender", "Content", "Created At"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);
        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(">>")
            .row_highlight_style(selected_style);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}

#[derive(Debug, Clone, Default)]
struct OrderListWidget {
    state: Arc<RwLock<OrderListState>>,
}

#[derive(Debug, Default)]
struct OrderListState {
    orders: Vec<Order>,
    loading_state: LoadingState,
    table_state: TableState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
}

impl OrderListWidget {
    /// Start fetching the orders in the background.
    ///
    /// This method spawns a background task that fetches the orders from the Nostr relay.
    fn run(&self, client: Client) {
        let this = self.clone();
        tokio::spawn(this.fetch_orders(client));
    }

    async fn fetch_orders(self, client: Client) {
        self.set_loading_state(LoadingState::Loading);

        client
            .handle_notifications(move |notification| {
                let this = self.clone();
                async move {
                    if let RelayPoolNotification::Event {
                        subscription_id,
                        event,
                        ..
                    } = notification
                    {
                        if subscription_id == SubscriptionId::new("orders-sub-id") {
                            this.handle_order_event(*event)?;
                        }
                    }
                    Ok(false)
                }
            })
            .await
            .unwrap();
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    fn scroll_down(&self) {
        self.state.write().unwrap().table_state.scroll_down_by(1);
    }

    fn scroll_up(&self) {
        self.state.write().unwrap().table_state.scroll_up_by(1);
    }

    fn handle_order_event(&self, event: nostr_sdk::Event) -> Result<()> {
        let order = order_from_tags(event)?;
        let mut state = self.state.write().unwrap();
        state.orders.retain(|o| o.id != order.id);

        if order.status == Some(Status::Pending) {
            state.orders.push(order);
        }

        state.loading_state = LoadingState::Loaded;
        if !state.orders.is_empty() {
            state.table_state.select(Some(0));
        }

        Ok(())
    }
}

impl Widget for &OrderListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // A block with a right-aligned title with the loading state on the right
        let loading_state = Line::from(format!("{:?}", state.loading_state)).right_aligned();
        let color: Color = Color::from_str("#1D212C").unwrap();
        let block = Block::bordered()
            .title(" Orders ")
            .title(loading_state)
            .bg(color)
            .title_bottom("j/k to scroll, ENTER to select order, q to quit");

        // A table with the list of orders
        let rows = state.orders.iter().map(|order| {
            let amount = order.sats_amount();
            let fiat_amount = order.fiat_amount();
            Row::new(vec![
                order.kind.unwrap().to_string(),
                order.fiat_code.clone(),
                amount,
                fiat_amount,
                order.payment_method.clone(),
                order.premium.to_string(),
            ])
        });
        let widths = [
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Length(15),
            Constraint::Fill(1),
            Constraint::Length(3),
        ];
        let color = Color::from_str("#304F00").unwrap();
        let header_style = Style::default().fg(SLATE.c200).bg(color);
        let selected_style = Style::default().fg(BLUE.c400);
        let header = [
            "Kind",
            "Code",
            "Amount",
            "Fiat Amount",
            "Payment Method",
            "+/-",
        ]
        .into_iter()
        .map(Cell::from)
        .collect::<Row>()
        .style(header_style)
        .height(1);
        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(">>")
            .row_highlight_style(selected_style);

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
