use chrono::{DateTime, Local, TimeZone};
use mostro_core::order::{Kind as OrderKind, Status};
use mostro_core::NOSTR_REPLACEABLE_EVENT_KIND;
use mostrui::my_order::Order;
use mostrui::util::order_from_tags;
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
use std::str::FromStr;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tui_textarea::TextArea;

#[tokio::main]
async fn main() -> Result<()> {
    let terminal = ratatui::init();
    let app = App::new();

    let client = Client::new(&app.my_keys);
    client.add_relay("wss://relay.mostro.network").await?;
    client.connect().await;

    let since = chrono::Utc::now() - chrono::Duration::days(1);
    let timestamp = since.timestamp();
    let since = Timestamp::from_secs(timestamp as u64);

    let filter = Filter::new()
        .kind(Kind::ParameterizedReplaceable(NOSTR_REPLACEABLE_EVENT_KIND))
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), vec!["mostro"])
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"])
        .since(since);
    client.subscribe(vec![filter], None).await?;

    let app_result = app.run(terminal, client).await;
    ratatui::restore();
    app_result
}

#[derive(Debug)]
struct App<'a> {
    my_keys: Keys,
    should_quit: bool,
    show_order: bool,
    selected_tab: usize,
    orders: OrderListWidget,
    show_amount_input: bool,
    amount_input: TextArea<'a>,
    new_order: Option<Order>,
}

impl<'a> App<'a> {
    const FRAMES_PER_SECOND: f32 = 60.0;

    pub fn new() -> Self {
        let mut amount_input = TextArea::default();
        amount_input.set_block(
            Block::default()
                .title("Input amount")
                .borders(ratatui::widgets::Borders::ALL),
        );

        Self {
            my_keys: Keys::generate(),
            should_quit: false,
            show_order: false,
            selected_tab: 0,
            orders: OrderListWidget::default(),
            show_amount_input: false,
            amount_input,
            new_order: None,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal, client: Client) -> Result<()> {
        self.orders.run(client);

        let period = Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => { terminal.draw(|frame| self.draw(frame))?; },
                Some(Ok(event)) = events.next() => self.handle_event(&event),
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
            2 => self.render_text_tab(frame, body_area, "Messages"),
            3 => self.render_text_tab(frame, body_area, "Settings"),
            _ => {}
        }

        if self.show_amount_input {
            let popup_area = popup_area(frame.area(), 50, 20);
            let block = Block::bordered().title("Amount input").bg(Color::Black);
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

            let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });

            frame.render_widget(Clear, popup_area);
            frame.render_widget(paragraph, popup_area);

            // Render input
            frame.render_widget(
                &self.amount_input,
                Rect::new(popup_area.x, popup_area.y + 4, popup_area.width, 3),
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
            let color: Color = Color::from_str("#14161C").unwrap();
            let block = Block::bordered()
                .title("Order details".to_string())
                .bg(color)
                .title_bottom(format!("ESC to close, ENTER to {}", action));
            let sats_amount = order.sats_amount();
            let premium = match order.premium.cmp(&0) {
                Ordering::Equal => "No premium or discount".to_string(),
                Ordering::Less => format!("a {}% discount", order.premium),
                Ordering::Greater => format!("a {}% premium", order.premium),
            };
            let fiat_amount = order.fiat_amount();
            let created_at: DateTime<Local> = Local
                .timestamp_opt(order.created_at.as_u64() as i64, 0)
                .unwrap();
            let lines = vec![
                Line::raw(format!(
                    "Someone is buying sats for {} {} at {} with {}.",
                    fiat_amount, order.fiat_code, sats_amount, premium
                )),
                Line::raw(""),
                Line::raw(format!("The payment method is {}.", order.payment_method)),
                Line::raw(""),
                Line::raw(format!("Id: {}", order.id)),
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

    fn render_text_tab(&self, frame: &mut Frame, area: Rect, text: &str) {
        let text_line = Line::from(text).centered();
        frame.render_widget(text_line, area);
    }

    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Char('j') | KeyCode::Down => self.orders.scroll_down(),
                    KeyCode::Char('k') | KeyCode::Up => self.orders.scroll_up(),
                    KeyCode::Left => {
                        if self.selected_tab > 0 {
                            self.selected_tab -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if self.selected_tab < 3 {
                            self.selected_tab += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // We get the selected order and process it
                        let selected = self.orders.state.read().unwrap().table_state.selected();
                        let state = self.orders.state.read().unwrap();
                        let order = match selected {
                            Some(i) => state.orders.get(i).unwrap(),
                            None => return,
                        };
                        if self.show_amount_input {
                            // Procesar la entrada del textarea
                            let value = self.amount_input.lines()[0].parse::<i64>().unwrap_or(0);

                            if value >= order.min_amount.unwrap_or(10)
                                && value <= order.max_amount.unwrap_or(500)
                            {
                                // Si el valor está dentro del rango permitido, procesa la orden
                                self.show_amount_input = false;
                                self.show_order = false;
                                println!("Order processed!");
                            } else {
                                // Aquí puedes mostrar un mensaje de error si es necesario
                            }
                        } else if self.show_order {
                            if order.max_amount.is_some() {
                                // Show range popup
                                self.show_amount_input = true;
                                self.show_order = false;
                            } else {
                                // Show popup of order detail
                                self.show_order = true;
                            }
                        } else {
                            // Mostrar popup de detalles de la orden
                            self.show_order = true;
                        }
                    }
                    KeyCode::Esc => self.show_order = false,
                    _ => {
                        if self.show_amount_input {
                            self.amount_input.input(*key); // Manejar eventos de teclado en textarea
                        }
                    }
                }
            }
        }
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
                    if let RelayPoolNotification::Event { event, .. } = notification {
                        let order = order_from_tags(*event).unwrap();

                        let mut state = this.state.write().unwrap();
                        state.orders.retain(|o| o.id != order.id);

                        if order.status == Some(Status::Pending) {
                            state.orders.push(order);
                        }

                        state.loading_state = LoadingState::Loaded;
                        if !state.orders.is_empty() {
                            state.table_state.select(Some(0));
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
            .highlight_style(selected_style);

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
