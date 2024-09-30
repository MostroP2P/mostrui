use mostro_core::order::{Kind as OrderKind, Status};
use mostro_core::NOSTR_REPLACEABLE_EVENT_KIND;
use nostr_sdk::prelude::*;
use ratatui::layout::Flex;
use ratatui::style::Color;
use ratatui::widgets::Clear;
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
use std::str::FromStr;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

#[derive(Debug, Default, Clone)]
pub struct Order {
    id: String,
    kind: Option<OrderKind>,
    fiat_code: String,
    status: Option<Status>,
    amount: i64,
    min_amount: Option<i64>,
    max_amount: Option<i64>,
    fiat_amount: i64,
    payment_method: String,
    premium: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new(&Keys::generate());
    client.add_relay("wss://relay.mostro.network").await?;
    client.connect().await;

    let since = chrono::Utc::now() - chrono::Duration::days(1);
    let timestamp = since.timestamp();
    let since = Timestamp::from_secs(timestamp as u64);

    let filter = Filter::new()
        .kind(Kind::ParameterizedReplaceable(NOSTR_REPLACEABLE_EVENT_KIND))
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), vec!["mostro"])
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"])
        .since(since)
        .limit(10);
    client.subscribe(vec![filter], None).await?;

    let terminal = ratatui::init();
    let app_result = App::default().run(terminal, client).await;
    ratatui::restore();
    app_result
}

#[derive(Debug, Default)]
struct App {
    should_quit: bool,
    show_order: bool,
    selected_tab: usize,
    orders: OrderListWidget,
}

impl App {
    const FRAMES_PER_SECOND: f32 = 60.0;

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

    fn draw(&self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]);
        let [tabs_area, body_area] = vertical.areas(frame.area());

        // Defining tabs labels
        let tab_titles = ["Orders", "My Trades", "Messages", "Settings"]
            .iter()
            .map(|t| Line::from(*t))
            .collect::<Vec<Line>>();

        let tabs = Tabs::new(tab_titles)
            .block(Block::bordered().title("Menu"))
            .select(self.selected_tab)
            .highlight_style(Style::new().fg(Color::Yellow));

        frame.render_widget(tabs, tabs_area);

        match self.selected_tab {
            0 => self.render_orders_tab(frame, body_area),
            1 => self.render_text_tab(frame, body_area, "My Trades"),
            2 => self.render_text_tab(frame, body_area, "Messages"),
            3 => self.render_text_tab(frame, body_area, "Settings"),
            _ => {}
        }

        if self.show_order {
            let popup_area = popup_area(frame.area(), 80, 80);
            let selected = self.orders.state.read().unwrap().table_state.selected();
            let state = self.orders.state.read().unwrap();
            let order = match selected {
                Some(i) => state.orders.get(i).unwrap(),
                None => return,
            };

            let block = Block::bordered().title(order.id.to_string());
            frame.render_widget(Clear, popup_area);
            frame.render_widget(block, popup_area);
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
                    KeyCode::Enter => self.show_order = true,
                    KeyCode::Esc => self.show_order = false,
                    _ => {}
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
                        let order = order_from_tags(event.tags).unwrap();

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
            .title("Orders")
            .title(loading_state)
            .bg(color)
            .title_bottom("j/k to scroll, q to quit");

        // A table with the list of orders
        let rows = state.orders.iter().map(|order| {
            let amount = if order.amount == 0 {
                "Market price".to_string()
            } else {
                order.amount.to_string()
            };
            let fiat_amount = if order.max_amount.is_some() {
                format!(
                    "{}-{}",
                    order.min_amount.unwrap(),
                    order.max_amount.unwrap()
                )
            } else {
                order.fiat_amount.to_string()
            };
            Row::new(vec![
                order.id.clone(),
                order.kind.unwrap().to_string(),
                order.fiat_code.clone(),
                amount,
                fiat_amount,
                order.payment_method.clone(),
                order.premium.to_string(),
            ])
        });
        let widths = [
            Constraint::Length(12),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Length(15),
            Constraint::Fill(1),
            Constraint::Length(3),
        ];
        let color = Color::from_str("#3E6601").unwrap();
        let header_style = Style::default().fg(SLATE.c200).bg(color);
        let selected_style = Style::default().fg(BLUE.c400);
        let header = [
            "Id",
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

pub fn order_from_tags(tags: Vec<Tag>) -> Result<Order> {
    let mut order = Order::default();
    for tag in tags {
        let t = tag.as_slice();
        let v = t.get(1).unwrap().as_str();
        match t.first().unwrap().as_str() {
            "d" => {
                order.id = v.to_string();
            }
            "k" => {
                order.kind = Some(OrderKind::from_str(v).unwrap());
            }
            "f" => {
                order.fiat_code = v.to_string();
            }
            "s" => {
                order.status = Some(Status::from_str(v).unwrap_or(Status::Dispute));
            }
            "amt" => {
                order.amount = v.parse::<i64>().unwrap();
            }
            "fa" => {
                if v.contains('.') {
                    continue;
                }
                let max = t.get(2);
                if max.is_some() {
                    order.min_amount = v.parse::<i64>().ok();
                    order.max_amount = max.unwrap().parse::<i64>().ok();
                } else {
                    let fa = v.parse::<i64>();
                    order.fiat_amount = fa.unwrap_or(0);
                }
            }
            "pm" => {
                order.payment_method = v.to_string();
            }
            "premium" => {
                order.premium = v.parse::<i64>().unwrap();
            }
            _ => {}
        }
    }

    Ok(order)
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
