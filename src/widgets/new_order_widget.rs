use nostr_sdk::bitcoin::Amount;
use ratatui::{
  buffer::Buffer,
  layout::{Constraint, Direction, Flex, Layout, Rect},
  style::{Color, Style, Stylize},
  widgets::{Block, Widget}, Frame,
};
use std::str::FromStr;
use mostro_core::order::Order;
use tui_prompts::prelude::*;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum Field {
    #[default]
    Kind,
    Currency,
    Amount,
}

#[derive(Debug,Default)]
pub struct NewOrderWidget<'a> {
  pub order: Order,
  current_field: Field,
  kind: TextState<'a>,
  currency: TextState<'a>,
  amount: TextState<'a>,

}

impl<'a> NewOrderWidget<'a> {
  pub fn new() -> Self {
      Self { ..Default::default() }
  }
  fn split_layout(&self, area: Rect) -> [Rect; 3] {
    let inner_area = create_layout(area, 50, 50);
    let areas = Layout::vertical([Constraint::Max(1), Constraint::Max(1), Constraint::Max(2)]).areas(inner_area);
    areas
  }
}

impl<'a> Widget for NewOrderWidget<'a> {
  fn render(self, area: Rect, buf: &mut Buffer) {
      let layout = create_layout(area,50,50);
      render_block(
          layout,
          buf,
          "Create new order",
      );
  }
}

pub fn draw_new_order_widget(frame: &mut Frame, area: Rect) {
  let mut new_order_widget = NewOrderWidget::new();

  let [kind_area, currency_area, amount_area] =
  new_order_widget.split_layout(area);

  TextPrompt::from("Order Kind").draw(frame, kind_area, &mut new_order_widget.kind);
  TextPrompt::from("Currency").draw(frame, currency_area, &mut new_order_widget.currency);
  TextPrompt::from("Amount").draw(frame, amount_area, &mut new_order_widget.amount);

  render_block(area, frame.buffer_mut(), "New Order");

}

  /// helper function to create a centered rect using up certain percentage of the available rect `r`
fn create_layout(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
  let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
  let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
  let [area] = vertical.areas(area);
  let [area] = horizontal.areas(area);

  area
}


fn render_block(area: Rect, buf: &mut Buffer, title: &str) {
  let block = Block::bordered().title(title).style(Style::new().black().on_green());
  let inner_area = block.inner(area);


  block.render(area, buf);

  // render_label_and_value(inner_area, buf, label);
}

fn render_label_and_value(inner_area: Rect, buf: &mut Buffer, label: &str) {
  let label_color = Style::default().fg(Color::from_str("#14161C").unwrap());
  let value_color = Style::default().fg(Color::White);

  buf.set_string(inner_area.x + 2, inner_area.y + 1, label, label_color);
  //buf.set_string(inner_area.x + 2, inner_area.y + 3, value, value_color);
}
