use ratatui::{
  buffer::Buffer,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, Widget},
};
use nostr_sdk::prelude::{PublicKey, SecretKey};
use nostr_sdk::ToBech32;
use std::str::FromStr;

pub struct SettingsWidget {
  pub pubkey: PublicKey,
  pub secret: SecretKey,
}

impl SettingsWidget {
  pub fn new(pubkey: PublicKey, secret: SecretKey) -> Self {
      Self { pubkey, secret }
  }
}

impl Widget for SettingsWidget {
  fn render(self, area: Rect, buf: &mut Buffer) {
      let layout = create_layout(area);
      render_block(
          layout[0],
          buf,
          "Mostro info ℹ️",
          "Public key of this mostro operator",
          &self.pubkey.to_bech32().unwrap(),
      );
      render_block(
          layout[1],
          buf,
          "Secret key 🔑",
          "Be mindful of this information",
          &self.secret.to_bech32().unwrap(),
      );
  }
}

fn create_layout(area: Rect) -> Vec<Rect> {
  Layout::default()
      .direction(Direction::Vertical)
      .constraints(
          [
              Constraint::Percentage(50),
              Constraint::Percentage(50),
          ]
          .as_ref(),
      )
      .split(area)
      .to_vec()
}

fn render_block(area: Rect, buf: &mut Buffer, title: &str, label: &str, value: &str) {
  let block = Block::bordered().title(title);
  let inner_area = block.inner(area);
  block.render(area, buf);

  render_label_and_value(inner_area, buf, label, value);
}

fn render_label_and_value(inner_area: Rect, buf: &mut Buffer, label: &str, value: &str) {
  let label_color = Style::default().fg(Color::from_str("#14161C").unwrap());
  let value_color = Style::default().fg(Color::White);

  buf.set_string(inner_area.x + 2, inner_area.y + 1, label, label_color);
  buf.set_string(inner_area.x + 2, inner_area.y + 3, value, value_color);
}
