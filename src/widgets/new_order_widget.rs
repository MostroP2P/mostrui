use ratatui::{
  buffer::Buffer, layout::{Constraint, Direction, Flex, Layout, Rect}, style::{Color, Style, Stylize}, text::{self, Text}, widgets::{Block, Borders, List, ListDirection, Paragraph, Widget}
};
use nostr_sdk::prelude::*;
use std::str::FromStr;
use mostro_core::order::Order;

use crate::popup_area;


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
      let layout = create_layout(area,80,60);
      render_block(
          layout,
          buf,
          "Create new order",
          "test",
      );
  }
}

// ANCHOR: centered_rect
/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
  // Cut the given rectangle into three vertical pieces
  let popup_layout = Layout::default()
      .direction(Direction::Vertical)
      .constraints([
          Constraint::Percentage((100 - percent_y) / 2),
          Constraint::Percentage(percent_y),
          Constraint::Percentage((100 - percent_y) / 2),
      ])
      .split(r);

  // Then cut the middle vertical piece into three width-wise pieces
  Layout::default()
      .direction(Direction::Horizontal)
      .constraints([
          Constraint::Percentage((100 - percent_x) / 2),
          Constraint::Percentage(percent_x),
          Constraint::Percentage((100 - percent_x) / 2),
      ])
      .split(popup_layout[1])[1] // Return the middle chunk
}

  /// helper function to create a centered rect using up certain percentage of the available rect `r`
fn create_layout(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
   let area = centered_rect(percent_x, percent_y, area);
        //frame.render_widget(popup_block, area);
        // ANCHOR_END: editing_popup
   
  area
}


fn render_block(area: Rect, buf: &mut Buffer, title: &str, label: &str) {
  let block = Block::bordered().title(title);
  let inner_area = block.inner(area);

  let popup_chunks = Layout::default()
  .direction(Direction::Horizontal)
  .margin(1)
  .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
  .split(inner_area);   

  let items = ["Kind (Sell/Buy)", "Currency", "Amount","Payment method","Premium"];
  let list = List::new(items)
    .block(Block::bordered().title("Oreder parameter"))
    .style(Style::new().white())
    .highlight_style(Style::new().italic())
    .highlight_symbol(">>")
    .repeat_highlight_symbol(true)
    .direction(ListDirection::TopToBottom);
  

  // ANCHOR: key_value_blocks
  // let key_block = Block::default().title("Order Kind").borders(Borders::ALL);
  // let value_block = Block::default().title("Amount").borders(Borders::ALL);
  // let currency_block = Block::default().title("Currency").borders(Borders::ALL);

  // let key_text = Paragraph::new(Text::styled("Hello, world!", Style::default())).block(key_block);
  // // 
  list.render(popup_chunks[0],buf);
  // value_block.render(popup_chunks[0],buf);
  // currency_block.render(popup_chunks[0],buf);


  block.render(area, buf);

  render_label_and_value(inner_area, buf, label);
}

fn render_label_and_value(inner_area: Rect, buf: &mut Buffer, label: &str) {
  let label_color = Style::default().fg(Color::from_str("#14161C").unwrap());
  let value_color = Style::default().fg(Color::White);

  buf.set_string(inner_area.x + 2, inner_area.y + 1, label, label_color);
  //buf.set_string(inner_area.x + 2, inner_area.y + 3, value, value_color);
}
