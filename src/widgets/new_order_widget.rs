use ratatui::{
  buffer::Buffer,
  layout::{Constraint, Direction, Flex, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, Widget},
};
use nostr_sdk::prelude::*;
use std::str::FromStr;
use mostro_core::order::Order;


#[derive(Debug,Default)]
pub struct NewOrderWidget {
  pub order: Order,
}

impl NewOrderWidget {
  pub fn new(order: Order) -> Self {
      Self { order }
  }
}

impl Widget for NewOrderWidget {
  fn render(self, area: Rect, buf: &mut Buffer) {
      let layout = create_layout(area,50,50);
      render_block(
          layout,
          buf,
          "Create new order",
          "test",
      );
  }
}

  /// helper function to create a centered rect using up certain percentage of the available rect `r`
fn create_layout(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
  let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
  let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
  let [area] = vertical.areas(area);
  let [area] = horizontal.areas(area);
  area
}


fn render_block(area: Rect, buf: &mut Buffer, title: &str, label: &str) {
  let block = Block::bordered().title(title);
  let inner_area = block.inner(area);
  block.render(area, buf);

  render_label_and_value(inner_area, buf, label);
}

fn render_label_and_value(inner_area: Rect, buf: &mut Buffer, label: &str) {
  let label_color = Style::default().fg(Color::from_str("#14161C").unwrap());
  let value_color = Style::default().fg(Color::White);

  buf.set_string(inner_area.x + 2, inner_area.y + 1, label, label_color);
  //buf.set_string(inner_area.x + 2, inner_area.y + 3, value, value_color);
}
